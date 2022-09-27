use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::caching::{Cache, CacheError};
use crate::id::{CommentId, Id};
use crate::source::{DataSource, Property};

/// Vector of Strings containing tags
pub type Tags = Vec<String>;

/// A Comment on an issue
#[derive(Debug, Eq)]
pub struct Comment {
    id: CommentId,
    author: String,
    cdate: OffsetDateTime,
    body: String,
}

impl PartialEq for Comment {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Comment {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Comment {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cdate.cmp(&other.cdate)
    }
}

impl Comment {
    /// Create new instance
    #[inline]
    #[must_use]
    pub const fn new(id: CommentId, author: String, cdate: OffsetDateTime, body: String) -> Self {
        Self {
            id,
            author,
            cdate,
            body,
        }
    }

    /// Return comment id as &str
    #[inline]
    #[must_use]
    pub fn id(&self) -> &str {
        self.id.id()
    }

    /// Return body text
    #[inline]
    #[must_use]
    pub fn body(&self) -> &str {
        &*self.body
    }

    /// Get a reference to the comment's author.
    #[must_use]
    #[inline]
    pub fn author(&self) -> &str {
        self.author.as_ref()
    }

    /// Get the comment's cdate.
    #[inline]
    #[must_use]
    pub const fn cdate(&self) -> OffsetDateTime {
        self.cdate
    }
}

pub type Cdate = OffsetDateTime;
pub type Ddate = OffsetDateTime;

#[derive(Debug)]
enum PlaceHolders {
    CreationDate,
    DueDate,
    Description,
    Id,
    Milestone,
    Tags,
    ShortId,
    Text(String),
}

/// Format string pattern
#[derive(Debug)]
pub struct FormatString(Vec<PlaceHolders>);

impl TryFrom<&'_ str> for FormatString {
    type Error = String;

    #[inline]
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut result = vec![];
        let mut cur: String = String::new();
        let format_string = match value {
            "simple" => "%i %D",
            "oneline" => "ID: %i  Date: %c  Tags: %T  Desc: %D",
            "short" => "ID: %i%nDate: %c%nDue Date: %d%nTags: %T%nDescription: %D",
            f => f,
        };
        let mut chars = format_string.chars();
        while let Some(c) = chars.next() {
            if c == '%' {
                match chars.next() {
                    Some('%') => {
                        cur.push('%');
                    }
                    Some('n') => {
                        cur.push('\n');
                    }
                    Some(n) => {
                        if !cur.is_empty() {
                            result.push(PlaceHolders::Text(cur.clone()));
                        }
                        match n {
                            'i' => {
                                result.push(PlaceHolders::ShortId);
                            }
                            'I' => {
                                result.push(PlaceHolders::Id);
                            }
                            'D' => {
                                result.push(PlaceHolders::Description);
                            }
                            'M' => {
                                result.push(PlaceHolders::Milestone);
                            }
                            'c' => {
                                result.push(PlaceHolders::CreationDate);
                            }
                            'd' => {
                                result.push(PlaceHolders::DueDate);
                            }
                            'T' => {
                                result.push(PlaceHolders::Tags);
                            }
                            _ => {
                                return Err(format!(
                                    "Unexpected formatstring place holder '{}{}'",
                                    c, n
                                ));
                            }
                        }
                        if !cur.is_empty() {
                            cur = String::new();
                        }
                    }
                    None => {
                        return Err("Premature end of string. Expected placeholder".to_owned());
                    }
                }
            } else {
                cur.push(c);
            }
        }
        if !cur.is_empty() {
            result.push(PlaceHolders::Text(cur));
        }
        Ok(Self(result))
    }
}

impl FormatString {
    /// Return issue formatted as string
    #[inline]
    pub fn format(&self, issue: &mut Issue<'_>) -> String {
        let mut result = String::new();
        for ph in &self.0 {
            let text = match ph {
                PlaceHolders::CreationDate => {
                    if let Err(e) = issue.cache_cdate() {
                        log::error!("creation date for id({}) {}", e, issue.id().short_id());
                        String::default()
                    } else {
                        issue.cdate().to_string()
                    }
                }
                PlaceHolders::DueDate => {
                    if let Err(e) = issue.cache_ddate() {
                        log::error!("due date for id({}) {}", e, issue.id().short_id());
                        String::default()
                    } else {
                        issue.ddate().map(|v| v.to_string()).unwrap_or_default()
                    }
                }
                PlaceHolders::Description => {
                    if let Err(e) = issue.cache_desc() {
                        log::error!("desc for id({}) {}", e, issue.id().short_id());
                        String::default()
                    } else {
                        issue.title()
                    }
                }
                PlaceHolders::Id => issue.id().id().clone(),
                PlaceHolders::ShortId => issue.id().short_id().to_owned(),
                PlaceHolders::Milestone => {
                    if let Err(e) = issue.cache_milestone() {
                        log::error!("milestone for id({}) {}", e, issue.id().short_id());
                        String::default()
                    } else {
                        issue
                            .milestone()
                            .as_ref()
                            .map_or_else(String::default, ToString::to_string)
                    }
                }
                PlaceHolders::Tags => {
                    if let Err(e) = issue.cache_tags() {
                        log::error!("tags for id({}) {}", e, issue.id().short_id());
                        String::default()
                    } else {
                        issue.tags().join(" ")
                    }
                }
                PlaceHolders::Text(t) => t.to_string(),
            };
            result.push_str(&text);
        }
        result
    }
}

/// Represents an issue
#[derive(Debug)]
pub struct Issue<'src> {
    id: Id,
    inner_cdate: Cache<Cdate>,
    inner_comments: Cache<Vec<Comment>>,
    inner_ddate: Cache<Option<Ddate>>,
    inner_desc: Cache<String>,
    inner_milestone: Cache<Option<String>>,
    inner_tags: Cache<Tags>,
    src: &'src DataSource,
}

impl<'src> Issue<'src> {
    /// Create new Issue from `Id`
    #[inline]
    #[must_use]
    pub const fn new(src: &'src DataSource, id: Id) -> Issue<'src> {
        Issue {
            id,
            inner_cdate: None,
            inner_comments: None,
            inner_ddate: None,
            inner_desc: None,
            inner_milestone: None,
            inner_tags: None,
            src,
        }
    }

    /// Return the Issue id
    #[inline]
    #[must_use]
    pub const fn id(&self) -> &Id {
        &self.id
    }

    /// Return `true` if issue is closed
    #[inline]
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.tags().contains(&"closed".to_owned())
    }

    /// Cache the creation date data
    ///
    /// # Errors
    ///
    /// Error during caching
    #[inline]
    pub fn cache_cdate(&mut self) -> Result<&mut Self, CacheError> {
        if self.inner_cdate.is_none() {
            let mut cmd = self.src.repo.git();
            cmd.args(&["show", "--no-patch", "--format=%aI", self.id().id()]);
            let out = cmd.output()?;

            let output = String::from_utf8_lossy(&out.stdout);
            let date_text = output.trim();
            self.inner_cdate =
                Some(OffsetDateTime::parse(date_text, &Rfc3339).expect("Valid RFC-3339 date"));
        }
        Ok(self)
    }

    /// Cache the creation date data
    ///
    /// # Errors
    ///
    /// Error during caching
    #[inline]
    pub fn cache_comments(&mut self) -> Vec<Result<(), CacheError>> {
        if self.inner_comments.is_none() {
            let (succesfull, failures): (Vec<_>, Vec<_>) = self
                .src
                .comments(&self.id)
                .into_iter()
                .partition(Result::is_ok);
            let comments = succesfull.into_iter().map(Result::unwrap).collect();
            self.inner_comments = Some(comments);
            failures
                .into_iter()
                .map(Result::unwrap_err)
                .map(|e| Err(CacheError::from(e)))
                .collect()
        } else {
            vec![Ok(())]
        }
    }

    /// Cache the due date data
    ///
    /// # Errors
    ///
    /// Error during caching
    #[inline]
    pub fn cache_ddate(&mut self) -> Result<&mut Self, CacheError> {
        if self.inner_ddate.is_none() {
            self.inner_ddate = Some(
                if let Ok(date_text) = self.src.read(self.id(), &Property::DueDate) {
                    Some(OffsetDateTime::parse(&date_text, &Rfc3339).expect("Valid RFC-3339 date"))
                } else {
                    None
                },
            );
        }
        Ok(self)
    }

    /// Cache the description data
    ///
    /// # Errors
    ///
    /// Error during caching
    #[inline]
    pub fn cache_desc(&mut self) -> Result<&mut Self, CacheError> {
        if self.inner_desc.is_none() {
            let desc = self.src.read(self.id(), &Property::Description)?;
            self.inner_desc = Some(desc);
        }
        Ok(self)
    }

    /// Cache the milestone data
    ///
    /// # Errors
    ///
    /// Error during caching
    #[inline]
    pub fn cache_milestone(&mut self) -> Result<&mut Self, CacheError> {
        if self.inner_milestone.is_none() {
            let milestone = self.src.read(self.id(), &Property::Milestone).ok();
            self.inner_milestone = Some(milestone);
        }
        Ok(self)
    }

    /// Cache the tags data
    ///
    /// # Errors
    ///
    /// Error during caching
    #[inline]
    pub fn cache_tags(&mut self) -> Result<&mut Self, CacheError> {
        if self.inner_tags.is_none() {
            let tags = self.src.read(self.id(), &Property::Tags).map(|v| {
                v.trim()
                    .lines()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })?;
            self.inner_tags = Some(tags);
        }
        Ok(self)
    }

    /// Return the issue creation date
    #[inline]
    #[must_use]
    pub fn cdate(&self) -> &'_ Cdate {
        self.inner_cdate.as_ref().expect("Cached creation date")
    }

    /// Return issue comments
    #[inline]
    #[must_use]
    pub fn comments(&self) -> &'_ Vec<Comment> {
        self.inner_comments.as_ref().expect("Cached comments")
    }

    /// Return the issue due date
    #[inline]
    #[must_use]
    pub fn ddate(&self) -> &Option<Ddate> {
        self.inner_ddate.as_ref().expect("Cached ddate")
    }

    /// Return the issue description
    #[inline]
    #[must_use]
    pub fn desc(&self) -> &'_ String {
        self.inner_desc.as_ref().expect("Cached description")
    }

    /// Return the issue milestone
    #[inline]
    #[must_use]
    pub fn milestone(&self) -> &'_ Option<String> {
        self.inner_milestone.as_ref().expect("Cached milestone")
    }

    /// Return the issue tagsription
    #[inline]
    #[must_use]
    pub fn tags(&self) -> &'_ Tags {
        self.inner_tags.as_ref().expect("Cached tags")
    }

    /// # Errors
    ///
    /// Will throw error on failure to read from description file
    #[inline]
    #[must_use]
    pub fn title(&self) -> String {
        self.desc().lines().next().unwrap_or("").to_owned()
    }
}
