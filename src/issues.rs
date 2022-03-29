use chrono::{DateTime, FixedOffset};

use crate::id::Id;
use crate::source::{DataSource, Property};

/// Vector of Strings containing tags
pub type Tags = Vec<String>;

type Cache<T> = Option<T>;
pub type Cdate = DateTime<FixedOffset>;
pub type Ddate = DateTime<FixedOffset>;
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
                    issue.cache_cdate().expect("cache cdate");
                    issue.cdate().to_string()
                }
                PlaceHolders::DueDate => {
                    issue.cache_ddate().expect("cached duedate");
                    issue.ddate().map(|v| v.to_string()).unwrap_or_default()
                }
                PlaceHolders::Description => {
                    issue.cache_desc().expect("cached description");
                    issue.title()
                }
                PlaceHolders::Id => issue.id().id().to_owned(),
                PlaceHolders::ShortId => issue.id().short_id().to_owned(),
                PlaceHolders::Milestone => {
                    issue.cache_milestone().expect("cache milestone");
                    issue
                        .milestone()
                        .as_ref()
                        .map_or_else(String::default, ToString::to_string)
                }
                PlaceHolders::Tags => {
                    issue.cache_tags().expect("cached tags");
                    issue.tags().join(" ")
                }
                PlaceHolders::Text(t) => t.to_string(),
            };
            result.push_str(&text);
        }
        result
    }
}

/// Error during caching
#[allow(missing_docs)]
#[derive(thiserror::Error, Debug)]
pub enum CacheError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Represents an issue
#[derive(Debug)]
pub struct Issue<'src> {
    id: Id,
    inner_cdate: Cache<Cdate>,
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
                Some(DateTime::parse_from_rfc3339(date_text).expect("Valid DateTime"));
        }
        Ok(self)
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
                    Some(DateTime::parse_from_rfc3339(&date_text).expect("Valid DateTime"))
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