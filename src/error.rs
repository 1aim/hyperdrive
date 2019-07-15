use crate::{BoxedError, DefaultFuture};
use futures::IntoFuture;
use http::StatusCode;
use std::{
    borrow::Cow,
    error,
    error::Error as StdError,
    fmt::{self, Display},
};

/// Error type to use if your `Guard`/`FromBody` implementation has no custom error.
///
/// This also includes cases where the [`Guard`]/[`FromBody`] can not fail.
/// For now this just aliases `hyper::Error` as `FromRequest` implementations
/// can always run into `hyper::Error`'s and as such any custom error type
/// needs to implement `From<hyper::Error>`.
///
/// [`Guard`]: trait.Guard.html
/// [`FromBody`]: trait.FromBody.html
pub type NoCustomError = hyper::Error;

/// Error returned by [`FromRequest`] implementations and [`FromBody`] implementations.
///
/// [`FromRequest`]: trait.FromRequest.html
/// [`FromBody`]: trait.FromBody.html
#[derive(Debug)]
pub enum FromRequestError<E>
where
    E: 'static,
{
    /// Custom error which can be returned by [`Guard`]'s and route handlers.
    ///
    /// *Be aware that `hyper` normally handles `Service` errors by cutting of
    /// the connection and not logging any error.* (Which is the appropriate
    /// behavior for protocol level and lower errors like a physical disconnect
    /// or the client starting to send random bytes). If you want different
    /// error handling you have to do it yourself, e.g. by wrapping your hyper
    /// service.
    ///
    /// While this error type has no bounds on the contained custom error some
    /// places (mainly [`FromRequest`]) restrict it to errors which have following
    /// bound: `std::error::Error + From<hyper::Error> + Send + Sync + 'static`.
    /// Additionally `std::error::Error` is only implemented if `E` implements
    /// it (same goes for `Debug` and `Display`).
    ///
    /// For cases where no custom error is needed the [`NoCustomError`] type is
    /// normally used.
    ///
    /// [`Guard`]: trait.Guard.html
    /// [`FromRequest`]: trait.FromRequest.html
    /// [`NoCustomError`]: type.NoCustomError.html
    Custom(E),

    /// [`BuildInError`] used by generated [`FromRequest`] implementations.
    ///
    /// [`SyncService`] and [`AsyncService`] will automatically convert this
    /// error to a appropriate HTTP response.
    ///
    /// [`BuildInError`]: struct.BuildInError.html
    /// [`FromRequest`]: trait.FromRequest.html
    /// [`SyncService`]: service/struct.SyncService.html
    /// [`AsyncService`]: service/struct.AsyncService.html
    BuildIn(BuildInError),
}

impl<E> FromRequestError<E>
where
    E: 'static,
{
    /// Creates a [`FromRequestError::BuildIn`] variant using [`BuildInError.malformed_body()`].
    ///
    /// [`FromRequestError::BuildIn`]: enum.FromRequestError.html#variant.BuildIn
    /// [`BuildInError.malformed_body()`]: struct.BuildInError.html#method.malformed_body
    pub fn malformed_body(source: BoxedError) -> Self {
        FromRequestError::BuildIn(BuildInError::malformed_body(source))
    }

    /// Creates a [`FromRequestError::BuildIn`] variant using [`BuildInErrorKind.wrong_method()`].
    ///
    /// [`FromRequestError::BuildIn`]: enum.FromRequestError.html#variant.BuildIn
    /// [`BuildInError.wrong_method()`]: struct.BuildInError.html#method.wrong_method
    pub fn wrong_method<M>(allowed_methods: M) -> Self
    where
        M: Into<Cow<'static, [&'static http::Method]>>,
    {
        FromRequestError::BuildIn(BuildInError::wrong_method(allowed_methods))
    }

    /// Creates a [`FromRequestError::BuildIn`] variant with a [`NoMatchingRoute`] error kind.
    ///
    /// [`FromRequestError::BuildIn`]: enum.FromRequestError.html#variant.BuildIn
    /// [`NoMatchingRoute`]: enum.BuildInErrorKind.html#variant.NoMatchingRoute
    pub fn no_matching_route() -> Self {
        let build_in = BuildInError::from_kind(BuildInErrorKind::NoMatchingRoute);
        FromRequestError::BuildIn(build_in)
    }

    /// If this error is a [`FromRequestError::BuildIn`] return a reference to it.
    ///
    /// [`FromRequestError::BuildIn`]: struct.FromRequestError.html#variant.BuildIn
    pub fn as_build_in(&self) -> Option<&BuildInError> {
        use self::FromRequestError::*;

        match self {
            BuildIn(err) => Some(err),
            _ => None,
        }
    }

    /// Return the contained [`BuildInError`] if there is one.
    ///
    /// [`BuildInError`]: struct.BuildInError.html
    pub fn into_build_in(self) -> Result<BuildInError, Self> {
        use self::FromRequestError::*;

        match self {
            BuildIn(err) => Ok(err),
            other => Err(other),
        }
    }

    /// If this error is a [`FromRequestError::Custom`] return a reference to it.
    ///
    /// [`FromRequestError::Custom`]: struct.FromRequestError.html#variant.Custom
    pub fn as_custom(&self) -> Option<&E> {
        use self::FromRequestError::*;

        match self {
            Custom(err) => Some(err),
            _ => None,
        }
    }

    /// Return the contained custom error if there is one.
    pub fn into_custom(self) -> Result<E, Self> {
        use self::FromRequestError::*;

        match self {
            Custom(err) => Ok(err),
            other => Err(other),
        }
    }

    /// Turns this error into a generic boxed future compatible with the output
    /// of `#[derive(FromRequest)]`.
    ///
    /// This is used by the code generated by `#[derive(FromRequest)]`.
    #[doc(hidden)] // not part of public API
    pub fn into_future<T: Send + 'static>(self) -> DefaultFuture<T, Self>
    where
        E: Send + 'static,
    {
        Box::new(Err(self).into_future())
    }

    /// Converts this error to a `FromRequestError<NewError>`.
    ///
    /// This is useful as transitive `From`/`Into` implementations tend to
    /// run into conflicting implementations problems.
    pub fn convert_custom_error<NewError>(self) -> FromRequestError<NewError>
    where
        E: Into<NewError>,
    {
        use self::FromRequestError::*;
        match self {
            Custom(err) => Custom(err.into()),
            BuildIn(err) => BuildIn(err),
        }
    }
}

impl<E> StdError for FromRequestError<E>
where
    E: StdError + 'static,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        use self::FromRequestError::*;
        match self {
            Custom(err) => Some(err),
            BuildIn(err) => Some(err),
        }
    }
}

impl<E> Display for FromRequestError<E>
where
    E: Display + 'static,
{
    fn fmt(&self, fter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::FromRequestError::*;
        match self {
            Custom(err) => Display::fmt(err, fter),
            BuildIn(err) => Display::fmt(err, fter),
        }
    }
}

/// The error type used by this library.
#[derive(Debug)]
pub struct BuildInError {
    kind: BuildInErrorKind,
    /// In case of a `WrongMethod` error, stores the allowed HTTP methods.
    allowed_methods: Cow<'static, [&'static http::Method]>,
    source: Option<BoxedError>,
}

impl BuildInError {
    /// Creates an error that contains just the given [`BuildInErrorKind`].
    ///
    /// [`BuildInErrorKind`]: enum.BuildInErrorKind.html
    pub fn from_kind(kind: BuildInErrorKind) -> Self {
        Self {
            kind,
            allowed_methods: (&[][..]).into(),
            source: None,
        }
    }

    /// Creates an error from an [`BuildInErrorKind`] and an underlying error that
    /// caused this one.
    ///
    /// # Parameters
    ///
    /// * **`kind`**: The [`BuildInErrorKind`] describing the error.
    /// * **`source`**: The underlying error that caused this one. Needs to
    ///   implement `Into<BoxedError>`, so any type that implements
    ///   `std::error::Error + Send + Sync` can be passed.
    ///
    /// [`BuildInErrorKind`]: enum.BuildInErrorKind.html
    pub fn with_source<S>(kind: BuildInErrorKind, source: S) -> Self
    where
        S: Into<BoxedError>,
    {
        Self {
            kind,
            allowed_methods: (&[][..]).into(),
            source: Some(source.into()),
        }
    }

    /// Creates an error with [`BuildInErrorKind::Body`].
    ///
    /// [`BuildInErrorKind::Body`]: enum.BuildInErrorKind.html#variant.Body
    pub fn wrong_method<M>(allowed_methods: M) -> Self
    where
        M: Into<Cow<'static, [&'static http::Method]>>,
    {
        Self {
            kind: BuildInErrorKind::WrongMethod,
            allowed_methods: allowed_methods.into(),
            source: None,
        }
    }

    /// Creates an error with [`BuildInErrorKind::WrongMethod`], given the allowed set
    /// of HTTP methods.
    ///
    /// [`BuildInErrorKind::WrongMethod`]: enum.BuildInErrorKind.html#variant.WrongMethod
    pub fn malformed_body(source: BoxedError) -> Self {
        Self {
            kind: BuildInErrorKind::Body,
            allowed_methods: (&[][..]).into(),
            source: Some(source),
        }
    }

    /// Returns the [`BuildInErrorKind`] that further describes this error.
    ///
    /// [`BuildInErrorKind`]: enum.BuildInErrorKind.html
    pub fn kind(&self) -> BuildInErrorKind {
        self.kind
    }

    /// Returns the HTTP status code that most closely describes this error.
    pub fn http_status(&self) -> StatusCode {
        self.kind.http_status()
    }

    /// Creates an HTTP response for indicating this error to the client.
    ///
    /// No body will be provided (hence the `()` body type), but the caller can
    /// `map` the result to supply one.
    ///
    /// # Example
    ///
    /// Call `map` on the response to supply your own HTTP payload:
    ///
    /// ```
    /// use hyperdrive::{Error, BuildInErrorKind};
    /// use hyper::Body;
    ///
    /// let error = Error::from_kind(BuildInErrorKind::NoMatchingRoute);
    /// let response = error.response()
    ///     .map(|()| Body::from("oh no!"));
    /// ```
    pub fn response(&self) -> http::Response<()> {
        let mut builder = http::Response::builder();
        builder.status(self.http_status());

        if self.kind == BuildInErrorKind::WrongMethod {
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

    /// If `self` is of type [`BuildInErrorKind::WrongMethod`], returns the list of
    /// allowed methods.
    ///
    /// Returns `None` if `self` is a different kind of error.
    ///
    /// [`BuildInErrorKind::WrongMethod`]: enum.BuildInErrorKind.html#variant.WrongMethod
    pub fn allowed_methods(&self) -> Option<&[&'static http::Method]> {
        if self.kind() == BuildInErrorKind::WrongMethod {
            Some(&self.allowed_methods)
        } else {
            None
        }
    }
}

impl fmt::Display for BuildInError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            None => write!(f, "{}", self.kind),
            Some(source) => write!(f, "{}: {}", self.kind, source),
        }
    }
}

impl error::Error for BuildInError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.source {
            Some(source) => Some(&**source),
            None => None,
        }
    }
}

/// The different kinds of errors that can occur when using this library.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BuildInErrorKind {
    /// Failed to parse query parameters.
    ///
    /// 400 Bad Request.
    QueryParam,
    /// Failed to deserialize the request body.
    ///
    /// 400 Bad Request.
    Body,
    /// Failed to parse path segment using `FromStr` implementation.
    ///
    /// 404 Not Found.
    PathSegment,
    /// No route matched the request URL.
    ///
    /// 404 Not Found.
    NoMatchingRoute,
    /// The invoked endpoint doesn't support this method (but does support a
    /// different one).
    ///
    /// 405 Method Not Allowed.
    WrongMethod,

    #[doc(hidden)]
    __Nonexhaustive,
}

impl BuildInErrorKind {
    /// Returns the HTTP status code that most closely describes this error.
    pub fn http_status(&self) -> StatusCode {
        match self {
            BuildInErrorKind::QueryParam | BuildInErrorKind::Body => StatusCode::BAD_REQUEST,
            BuildInErrorKind::PathSegment | BuildInErrorKind::NoMatchingRoute => {
                StatusCode::NOT_FOUND
            }
            BuildInErrorKind::WrongMethod => StatusCode::METHOD_NOT_ALLOWED,
            BuildInErrorKind::__Nonexhaustive => unreachable!("__Nonexhaustive must never exist"),
        }
    }
}

impl fmt::Display for BuildInErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BuildInErrorKind::PathSegment => "failed to parse data in path segment",
            BuildInErrorKind::QueryParam => "failed to parse query parameters",
            BuildInErrorKind::Body => "failed to parse request body",
            BuildInErrorKind::NoMatchingRoute => "requested route does not exist",
            BuildInErrorKind::WrongMethod => "method not supported on this endpoint",
            BuildInErrorKind::__Nonexhaustive => unreachable!("__Nonexhaustive must never exist"),
        })
    }
}
