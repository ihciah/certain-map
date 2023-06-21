/// Re-export macro.
pub use certain_map_macros::certain_map;
/// We use service_async's ParamRef as getter trait.
pub use service_async::ParamRef;
/// And use our own trait as setter trait.
/// But maybe moved to service-async later.
pub trait ParamSet<T> {
    type Transformed;
    fn set(self, item: T) -> Self::Transformed;
}
/// Use our own trait as remove trait.
/// But maybe moved to service-async later.
pub trait ParamRemove<T> {
    type Transformed;
    fn remove(self) -> Self::Transformed;
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Occupied<T>(pub T);

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Vacancy;
