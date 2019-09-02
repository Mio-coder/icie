#![feature(never_type)]

pub extern crate backtrace;
pub extern crate chrono;
pub extern crate debris;
pub extern crate html5ever;
pub extern crate log;
pub extern crate reqwest;
pub extern crate scraper;
pub extern crate selectors;
pub extern crate serde;

pub mod boxed;
#[macro_use]
pub mod statement;

use chrono::{DateTime, FixedOffset};
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Debug};

#[derive(Debug)]
pub enum Error {
	WrongCredentials,
	WrongData,
	WrongTaskUrl,
	AccessDenied,
	NotYetStarted,
	RateLimit,
	NetworkFailure(reqwest::Error),
	TLSFailure(reqwest::Error),
	URLParseFailure(reqwest::UrlError),
	UnexpectedHTML(debris::Error),
	UnexpectedJSON { endpoint: &'static str, backtrace: backtrace::Backtrace, resp_raw: String, inner: Option<Box<dyn std::error::Error+'static>> },
}
impl From<reqwest::Error> for Error {
	fn from(e: reqwest::Error) -> Self {
		Error::NetworkFailure(e)
	}
}
impl From<reqwest::UrlError> for Error {
	fn from(e: reqwest::UrlError) -> Self {
		Error::URLParseFailure(e)
	}
}
impl From<debris::Error> for Error {
	fn from(e: debris::Error) -> Self {
		Error::UnexpectedHTML(e)
	}
}
impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::WrongCredentials => f.write_str("wrong username or password"),
			Error::WrongData => f.write_str("wrong data passed to site API"),
			Error::WrongTaskUrl => f.write_str("wrong task URL format"),
			Error::AccessDenied => f.write_str("access denied"),
			Error::NotYetStarted => f.write_str("contest not yet started"),
			Error::RateLimit => f.write_str("rate limited due to too frequent network operations"),
			Error::NetworkFailure(_) => f.write_str("network failure"),
			Error::TLSFailure(_) => f.write_str("TLS encryption failure"),
			Error::URLParseFailure(_) => f.write_str("URL parse failure"),
			Error::UnexpectedHTML(_) => f.write_str("error when scrapping site API response"),
			Error::UnexpectedJSON { .. } => f.write_str("error when parsing site JSON response"),
		}
	}
}
impl std::error::Error for Error {
	fn source(&self) -> Option<&(dyn std::error::Error+'static)> {
		match self {
			Error::WrongCredentials => None,
			Error::WrongData => None,
			Error::WrongTaskUrl => None,
			Error::AccessDenied => None,
			Error::NotYetStarted => None,
			Error::RateLimit => None,
			Error::NetworkFailure(e) => Some(e),
			Error::TLSFailure(e) => Some(e),
			Error::URLParseFailure(e) => Some(e),
			Error::UnexpectedHTML(e) => Some(e),
			Error::UnexpectedJSON { inner, .. } => inner.as_ref().map(|bx| bx.as_ref()),
		}
	}
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub struct Example {
	pub input: String,
	pub output: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Statement {
	HTML {
		html: String,
	},
	PDF {
		#[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
		pdf: Vec<u8>,
	},
}

#[derive(Clone, Debug)]
pub struct TaskDetails {
	pub id: String,
	pub title: String,
	pub contest_id: String,
	pub site_short: String,
	pub examples: Option<Vec<Example>>,
	pub statement: Option<Statement>,
	pub url: String,
}

#[derive(Clone, Debug)]
pub struct Language {
	pub id: String,
	pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectionCause {
	WrongAnswer,
	RuntimeError,
	TimeLimitExceeded,
	MemoryLimitExceeded,
	RuleViolation,
	SystemError,
	CompilationError,
}

#[derive(Clone, Debug)]
pub struct Submission {
	pub id: String,
	pub verdict: Verdict,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
	Scored { score: f64, max: Option<f64>, cause: Option<RejectionCause>, test: Option<String> },
	Accepted,
	Rejected { cause: Option<RejectionCause>, test: Option<String> },
	Pending { test: Option<String> },
	Skipped,
}

#[derive(Clone, Debug)]
pub struct ContestDetails<I> {
	pub id: I,
	pub title: String,
	pub start: DateTime<FixedOffset>,
}

#[derive(Clone, Debug)]
pub enum Resource<C, T> {
	Contest(C),
	Task(T),
}

#[derive(Clone, Debug)]
pub struct URL<C, T> {
	pub domain: String,
	pub site: String,
	pub resource: Resource<C, T>,
}

impl URL<(), ()> {
	pub fn dummy_domain(domain: &str) -> URL<(), ()> {
		URL { domain: domain.to_owned(), site: format!("https://{}", domain), resource: Resource::Task(()) }
	}
}

pub trait Backend: Send+Sync+'static {
	type CachedAuth: Debug+Send+Sync+'static;
	type Contest: Debug+Send+Sync+'static;
	type Session: Debug+Send+Sync+'static;
	type Task: Debug+Send+Sync+'static;
	fn accepted_domains(&self) -> &'static [&'static str];
	fn deconstruct_resource(&self, domain: &str, segments: &[&str]) -> Result<Resource<Self::Contest, Self::Task>>;
	fn deconstruct_url(&self, url: &str) -> Result<Option<URL<Self::Contest, Self::Task>>> {
		let url: Url = url.parse()?;
		let domain = url.domain().ok_or(Error::WrongTaskUrl)?;
		if self.accepted_domains().contains(&domain) {
			let segments = url.path_segments().ok_or(Error::WrongTaskUrl)?.filter(|s| !s.is_empty()).collect::<Vec<_>>();
			let resource = self.deconstruct_resource(domain, &segments)?;
			Ok(Some(URL { domain: domain.to_owned(), site: format!("https://{}", domain), resource }))
		} else {
			Ok(None)
		}
	}
	fn connect(&self, client: reqwest::Client, domain: &str) -> Self::Session;
	fn auth_cache(&self, session: &Self::Session) -> Result<Option<Self::CachedAuth>>;
	fn auth_deserialize(&self, data: &str) -> Result<Self::CachedAuth>;
	fn auth_login(&self, session: &Self::Session, username: &str, password: &str) -> Result<()>;
	fn auth_restore(&self, session: &Self::Session, auth: &Self::CachedAuth) -> Result<()>;
	fn auth_serialize(&self, auth: &Self::CachedAuth) -> String;
	fn task_details(&self, session: &Self::Session, task: &Self::Task) -> Result<TaskDetails>;
	fn task_languages(&self, session: &Self::Session, task: &Self::Task) -> Result<Vec<Language>>;
	fn task_submissions(&self, session: &Self::Session, task: &Self::Task) -> Result<Vec<Submission>>;
	fn task_submit(&self, session: &Self::Session, task: &Self::Task, language: &Language, code: &str) -> Result<String>;
	fn task_url(&self, session: &Self::Session, task: &Self::Task) -> String;
	fn contest_id(&self, contest: &Self::Contest) -> String;
	fn contest_site_prefix(&self) -> &'static str;
	fn contest_tasks(&self, session: &Self::Session, contest: &Self::Contest) -> Result<Vec<Self::Task>>;
	fn contest_url(&self, contest: &Self::Contest) -> String;
	fn contests(&self, session: &Self::Session) -> Result<Vec<ContestDetails<Self::Contest>>>;
	fn name_short(&self) -> &'static str;
	fn supports_contests(&self) -> bool;
}

fn as_base64<T: AsRef<[u8]>, S: Serializer>(buffer: &T, serializer: S) -> std::result::Result<S::Ok, S::Error> {
	serializer.serialize_str(&hex::encode(buffer.as_ref()))
}
fn from_base64<'d, D: Deserializer<'d>>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error> {
	<&str as Deserialize<'d>>::deserialize(deserializer).and_then(|buffer| hex::decode(buffer).map_err(|e| serde::de::Error::custom(e.to_string())))
}

pub fn deserialize_auth<'d, T: Deserialize<'d>>(data: &'d str) -> Result<T> {
	serde_json::from_str(data).map_err(|_| Error::WrongData)
}
pub fn serialize_auth<T: Serialize>(auth: &T) -> String {
	serde_json::to_string(auth).unwrap()
}
