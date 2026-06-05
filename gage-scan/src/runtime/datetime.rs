use std::cmp::Ordering;
use std::time::SystemTime;

use chrono::{DateTime as ChronoDateTime, Utc};
use rune::alloc::fmt::TryWrite;
use rune::runtime::Formatter;
use rune::{Any, ContextError, Module, item};

#[derive(Any, Clone)]
#[rune(item = ::gage)]
pub struct DateTime {
    #[rune(skip)]
    inner: ChronoDateTime<Utc>,
}

impl rune::alloc::prelude::TryClone for DateTime {
    fn try_clone(&self) -> Result<Self, rune::alloc::Error> {
        Ok(self.clone())
    }
}

impl DateTime {
    pub(crate) fn from_system_time(t: SystemTime) -> Self {
        DateTime { inner: t.into() }
    }

    #[rune::function(keep, path = Self::from_millis)]
    pub(crate) fn from_millis(ms: i64) -> Self {
        let inner = ChronoDateTime::from_timestamp_millis(ms)
            .expect("epoch millis within chrono representable range");
        DateTime { inner }
    }

    #[rune::function(keep, instance)]
    fn to_rfc3339(&self) -> String {
        self.inner.to_rfc3339()
    }

    #[rune::function(keep, instance)]
    fn millis(&self) -> i64 {
        self.inner.timestamp_millis()
    }

    #[rune::function(keep, instance, protocol = PARTIAL_EQ)]
    fn partial_eq(&self, rhs: &Self) -> bool {
        PartialEq::eq(&self.inner, &rhs.inner)
    }

    #[rune::function(keep, instance, protocol = EQ)]
    fn eq(&self, rhs: &Self) -> bool {
        PartialEq::eq(&self.inner, &rhs.inner)
    }

    #[rune::function(keep, instance, protocol = PARTIAL_CMP)]
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        PartialOrd::partial_cmp(&self.inner, &rhs.inner)
    }

    #[rune::function(keep, instance, protocol = CMP)]
    fn cmp(&self, rhs: &Self) -> Ordering {
        Ord::cmp(&self.inner, &rhs.inner)
    }

    #[rune::function(keep, instance, protocol = DISPLAY_FMT)]
    fn display_fmt(&self, f: &mut Formatter) -> rune::alloc::Result<()> {
        write!(f, "{}", self.inner.to_rfc3339())
    }

    #[rune::function(keep, instance, protocol = DEBUG_FMT)]
    fn debug_fmt(&self, f: &mut Formatter) -> rune::alloc::Result<()> {
        write!(f, "{}", self.inner.to_rfc3339())
    }
}

pub(crate) fn register_types(m: &mut Module) -> Result<(), ContextError> {
    m.ty::<DateTime>()?;
    m.function_meta(DateTime::from_millis__meta)?;
    m.function_meta(DateTime::to_rfc3339__meta)?;
    m.function_meta(DateTime::millis__meta)?;
    m.function_meta(DateTime::partial_eq__meta)?;
    m.implement_trait::<DateTime>(item!(::std::cmp::PartialEq))?;
    m.function_meta(DateTime::eq__meta)?;
    m.implement_trait::<DateTime>(item!(::std::cmp::Eq))?;
    m.function_meta(DateTime::partial_cmp__meta)?;
    m.implement_trait::<DateTime>(item!(::std::cmp::PartialOrd))?;
    m.function_meta(DateTime::cmp__meta)?;
    m.implement_trait::<DateTime>(item!(::std::cmp::Ord))?;
    m.function_meta(DateTime::display_fmt__meta)?;
    m.function_meta(DateTime::debug_fmt__meta)?;
    Ok(())
}
