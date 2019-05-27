use crate::{BoxedError, DefaultFuture};
use futures::IntoFuture;
use http::StatusCode;
use std::{borrow::Cow, error, fmt};

/// The error type used by this library.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    /// In case of a `WrongMethod` error, stores the allowed HTTP methods.
    allowed_methods: Cow<'static, [&'static http::Method]>,
    source: Option<BoxedError>,
}

impl Error {
    /// Creates an error that contains just the given `ErrorKind`.
    pub fn from_kind(kind: ErrorKind) -> Self {
        Self {
            kind,
            allowed_methods: (&[][..]).into(),
            source: None,
        }
    }

    /// Creates an error from an `ErrorKind` and the underlying error that
    /// caused this one.
    pub fn with_source<S>(kind: ErrorKind, source: S) -> Self
    where
        S: Into<BoxedError>,
    {
        Self {
            kind,
            allowed_methods: (&[][..]).into(),
            source: Some(source.into()),
        }
    }

    /// Creates a `WrongMethod` error, given the allowed set of HTTP methods.
    pub fn wrong_method<M>(allowed_methods: M) -> Self
    where
        M: Into<Cow<'static, [&'static http::Method]>>,
    {
        Self {
            kind: ErrorKind::WrongMethod,
            allowed_methods: allowed_methods.into(),
            source: None,
        }
    }

    /// Returns the `ErrorKind` that further describes this error.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the HTTP status code that most closely describes this error.
    pub fn http_status(&self) -> StatusCode {
        self.kind.http_status()
    }

    /// Creates an HTTP response for indicating this error to the client.
    ///
    /// No body will be provided (`()`), but the caller can `map` the result to
    /// supply one.
    pub fn response(&self) -> http::Response<()> {
        let mut builder = http::Response::builder();
        builder.status(self.http_status());

        if self.kind == ErrorKind::WrongMethod {
            // The spec mandates that "405 Method Not Allowed" always sends an
            // `Allow` header
            let allowed = self
                .allowed_methods
                .iter()
                .map(|method| method.as_str().to_uppercase())
                .collect::<Vec<_>>()
                .join(", ");
            builder.header(http::header::ALLOW, allowed);
        }

        builder
            .body(())
            .expect("could not build HTTP response for error")
    }

    /// Turns this error into a generic boxed future compatible with the output
    /// of `#[derive(FromRequest)]`.
    ///
    /// This is used by the code generated by `#[derive(FromRequest)]`.
    pub fn into_future<T: Send + 'static>(self) -> DefaultFuture<T, BoxedError> {
        Box::new(Err(BoxedError::from(self)).into_future())
    }

    /// If `self` is of type `ErrorKind::WrongMethod`, returns the list of
    /// allowed methods.
    ///
    /// Returns `None` if `self` is a different kind of error.
    pub fn allowed_methods(&self) -> Option<&[&'static http::Method]> {
        if self.kind() == ErrorKind::WrongMethod {
            Some(&self.allowed_methods)
        } else {
            None
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            None => write!(f, "{}", self.kind),
            Some(source) => write!(f, "{}: {}", self.kind, source),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.source {
            Some(source) => Some(&**source),
            None => None,
        }
    }
}

/// The different kinds of errors that can occur when using this library.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Failed to parse query params. 400 Bad Request.
    QueryParam,
    /// Failed to deserialize the body. 400 Bad Request.
    Body,
    /// Failed to parse path segment using `FromStr` impl. 404 Not Found.
    PathSegment,
    /// No route matched the request URL. 404 Not Found.
    NoMatchingRoute,
    /// Endpoint doesn't support this method (but does support a different one).
    /// 405 Method Not Allowed.
    WrongMethod,

    #[doc(hidden)]
    __Nonexhaustive,
}

impl ErrorKind {
    /// Returns the HTTP status code that most closely describes this error.
    pub fn http_status(&self) -> StatusCode {
        match self {
            ErrorKind::QueryParam | ErrorKind::Body => StatusCode::BAD_REQUEST,
            ErrorKind::PathSegment | ErrorKind::NoMatchingRoute => StatusCode::NOT_FOUND,
            ErrorKind::WrongMethod => StatusCode::METHOD_NOT_ALLOWED,
            ErrorKind::__Nonexhaustive => unreachable!("__Nonexhaustive must never exist"),
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ErrorKind::PathSegment => "failed to parse data in path segment",
            ErrorKind::QueryParam => "failed to parse query parameters",
            ErrorKind::Body => "failed to parse request body",
            ErrorKind::NoMatchingRoute => "requested route does not exist",
            ErrorKind::WrongMethod => "method not supported on this endpoint",
            ErrorKind::__Nonexhaustive => unreachable!("__Nonexhaustive must never exist"),
        })
    }
}
