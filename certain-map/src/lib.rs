/// Re-export macro.
pub use certain_map_macros::certain_map;
/// We use service_async's Param*.
pub use service_async::{
    Param, ParamMaybeMut, ParamMaybeRef, ParamMut, ParamRef, ParamRemove, ParamSet, ParamTake,
};

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Occupied<T>(pub T);

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Vacancy;
