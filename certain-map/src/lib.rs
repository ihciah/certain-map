/// Re-export macro.
pub use certain_map_macros::certain_map;
/// We use service_async's ParamRef as getter trait.
pub use service_async::{Param, ParamMut, ParamRef};
/// And use our own trait as setter trait.
/// But maybe moved to service-async later.
pub trait ParamSet<T> {
    type Transformed;
    fn param_set(self, item: T) -> Self::Transformed;
}
/// Use our own trait as remove trait.
/// But maybe moved to service-async later.
pub trait ParamRemove<T> {
    type Transformed;
    fn param_remove(self) -> Self::Transformed;
}

/// Use our own trait as remove trait.
/// But maybe moved to service-async later.
pub trait ParamTake<T> {
    type Transformed;
    fn param_take(self) -> (Self::Transformed, T);
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Occupied<T>(pub T);

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Vacancy;
