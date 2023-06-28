/// Re-export macro.
pub use certain_map_macros::certain_map;
/// We use param's Param*.
pub use param::{
    Param, ParamMaybeMut, ParamMaybeRef, ParamMut, ParamRef, ParamRemove, ParamSet, ParamTake,
};

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Occupied<T>(pub T);

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Vacancy;
