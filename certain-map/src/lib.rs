#![doc = include_str!("../README.md")]

use std::mem::MaybeUninit;

/// Re-export macro.
pub use certain_map_macros::certain_map;
/// Item of type T has been set in a certain_map slot.
///
/// When used as a trait bound, `Param<T>` ensures that the constrained type has previously
/// used the [`ParamSet<T>`](trait.ParamSet.html) trait to set the value of type `T` in a
/// `certain_map` slot. This allows the caller to guarantee at compile-time that the value of
/// `T` is available and can be retrieved from the implementing type using the `param` method.
///
/// By using the `Param<T>` trait bound, you can enforce that the necessary value has been set
/// before attempting to retrieve it, preventing runtime errors caused by missing or
/// uninitialized values.
///
/// # Example
///
/// ```rust
/// fn process_param<P: Param<T>, T>(param_provider: P) {
///     let value: T = param_provider.param();
///     // Use the value of type T
/// }
/// ```
pub use param::Param;
/// Item of type T may have been set in a certain_map slot and returns Option<&mut T>.
///
/// When used as a trait bound, `ParamMaybeMut<T>` ensures that the constrained type implements
/// the `param_maybe_mut` method, which returns an `Option<&mut T>`. This allows the caller to
/// attempt to retrieve a mutable reference to the value of type `T` from the implementing
/// type, if it has been previously set in a `certain_map` slot using
/// [`ParamSet<T>`](trait.ParamSet.html).
///
/// By using the `ParamMaybeMut<T>` trait bound, you can handle cases where the value may or
/// may not have been set in the `certain_map`.
///
/// # Example
///
/// ```rust
/// fn process_param_maybe_mut<P: ParamMaybeMut<T>, T>(param_provider: &mut P) {
///     if let Some(value_mut) = param_provider.param_maybe_mut() {
///         // Modify the value of type T
///     }
/// }
/// ```
pub use param::ParamMaybeMut;
/// Item of type T may have been set in a certain_map slot and returns Option<&T>
///
/// When used as a trait bound, `ParamMaybeRef<T>` ensures that the constrained type implements
/// the `param_maybe_ref` method, which returns an `Option<&T>`. This allows the caller to
/// attempt to retrieve a reference to the value of type `T` from the implementing type, if it
/// has been previously set in a `certain_map` slot using [`ParamSet<T>`](trait.ParamSet.html).
///
/// The `ParamMaybeRef<T>` trait does not guarantee that the value has been set in the
/// `certain_map` slot. Instead, it returns an `Option<&T>`, which will be `Some(&T)` if the
/// value has been set in the `certain_map` slot, and `None` if the value has not been set.
///
/// # Example
///
/// ```rust
/// fn process_param_maybe_ref<P: ParamMaybeRef<T>, T>(param_provider: &P) {
///     if let Some(value_ref) = param_provider.param_maybe_ref() {
///         // Use the reference to the value of type T
///     }
/// }
/// ```
pub use param::ParamMaybeRef;
/// Item of type T has been set in a certain_map slot and returns a mutable reference.
///
/// When used as a trait bound, `ParamMut<T>` ensures that the constrained type has previously
/// used the [`ParamSet<T>`](trait.ParamSet.html) trait to set the value of type `T` in a
/// `certain_map` slot. This allows the caller to guarantee at compile-time that the value of
/// `T` is available and can be mutably accessed from the implementing type using the
/// `param_mut` method.
///
/// # Example
///
/// ```rust
/// fn process_param_mut<P: ParamMut<T>, T>(param_provider: &mut P) {
///     let value_mut: &mut T = param_provider.param_mut();
///     // Modify the value of type T
/// }
/// ```
pub use param::ParamMut;
/// Item of type T has been set in a certain_map slot and returns a reference.
///
/// When used as a trait bound, `ParamRef<T>` ensures that the constrained type implements the
/// `param_ref` method, which returns a reference to the value of type `T`. This allows the
/// caller to ensure that the value of `T` has been set in a `certain_map` slot and that a
/// reference to it can be retrieved.
///
/// # Example
///
/// ```rust
/// fn process_param_ref<P: ParamRef<T>, T>(param_provider: &P) {
///     let value_ref: &T = param_provider.param_ref();
///     // Use the reference to the value of type T
/// }
/// ```
pub use param::ParamRef;
/// Item of type T can be removed certain_map slot irrespective of it
/// having been set before.
pub use param::ParamRemove;
/// Item of type T is vacant in certain_map slot.
///
/// The `ParamSet<T>` trait transforms the struct when a value is set. If the slot
/// corresponding to the value of type `T` is currently of type [`Vacant`](struct.Vacant.html),
/// setting a value using `param_set` will transform it to [`Occupied`](struct.Occupied.html),
/// indicating that the value has been set. This transformation is reflected in the returned
/// Transformed type.
///
/// By using the `ParamSet<T>` as a trait bound, you can ensure that you are not overwriting a
/// field that has already been set. If you attempt to set a value in a slot that is already
/// [`Occupied`](struct.Occupied.html), the Rust compiler will raise an error, preventing
/// accidental overwrites and ensuring the integrity of the `certain_map` slots.
pub use param::ParamSet;
/// Item of type T has been set in certain_map slot and can be removed
/// from the slot, leaving it vacant.
pub use param::ParamTake;

/// Represents an occupied slot in a certain_map slot.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Occupied<T>(pub T);

/// Represents an occupied slot in a certain_map slot.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct OccupiedM;

/// Represents a vacant slot in a certain map.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Vacancy;

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::OccupiedM {}
    impl Sealed for super::Vacancy {}
}

pub trait MaybeAvailable: sealed::Sealed {
    /// # Safety
    /// Must called with correspond data reference.
    unsafe fn do_maybe_ref<T>(data: &MaybeUninit<T>) -> Option<&T>;
    /// # Safety
    /// Must called with correspond data reference.
    unsafe fn do_maybe_mut<T>(data: &mut MaybeUninit<T>) -> Option<&mut T>;
    /// # Safety
    /// Must called with correspond data reference and update state type.
    unsafe fn do_set<T>(data: &mut MaybeUninit<T>, value: T);
    /// # Safety
    /// Must called with correspond data reference and update state type.
    unsafe fn do_drop<T>(data: &mut MaybeUninit<T>);
    /// # Safety
    /// Must called with correspond data reference and update state type.
    unsafe fn do_clone<T: Clone>(data: &MaybeUninit<T>) -> MaybeUninit<T>;
    /// # Safety
    /// Must called with correspond data reference.
    unsafe fn do_debug<T: std::fmt::Debug>(
        data: &MaybeUninit<T>,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result;
}

pub trait Available: MaybeAvailable {
    /// # Safety
    /// Must called with correspond data reference.
    unsafe fn do_ref<T>(data: &MaybeUninit<T>) -> &T;
    /// # Safety
    /// Must called with correspond data reference.
    unsafe fn do_mut<T>(data: &mut MaybeUninit<T>) -> &mut T;
    /// # Safety
    /// Must called with correspond data reference and update state type.
    unsafe fn do_read<T: Clone>(data: &MaybeUninit<T>) -> T;
    /// # Safety
    /// Must called with correspond data reference and update state type.
    unsafe fn do_take<T>(data: &MaybeUninit<T>) -> T;
}

impl Available for OccupiedM {
    #[inline]
    unsafe fn do_ref<T>(data: &MaybeUninit<T>) -> &T {
        data.assume_init_ref()
    }
    #[inline]
    unsafe fn do_mut<T>(data: &mut MaybeUninit<T>) -> &mut T {
        data.assume_init_mut()
    }
    #[inline]
    unsafe fn do_read<T: Clone>(data: &MaybeUninit<T>) -> T {
        data.assume_init_ref().clone()
    }
    #[inline]
    unsafe fn do_take<T>(data: &MaybeUninit<T>) -> T {
        data.assume_init_read()
    }
}

impl MaybeAvailable for OccupiedM {
    #[inline]
    unsafe fn do_maybe_ref<T>(data: &MaybeUninit<T>) -> Option<&T> {
        Some(data.assume_init_ref())
    }

    #[inline]
    unsafe fn do_maybe_mut<T>(data: &mut MaybeUninit<T>) -> Option<&mut T> {
        Some(data.assume_init_mut())
    }

    #[inline]
    unsafe fn do_set<T>(data: &mut MaybeUninit<T>, value: T) {
        data.assume_init_drop();
        *data = MaybeUninit::new(value)
    }

    #[inline]
    unsafe fn do_drop<T>(data: &mut MaybeUninit<T>) {
        data.assume_init_drop()
    }

    #[inline]
    unsafe fn do_clone<T: Clone>(data: &MaybeUninit<T>) -> MaybeUninit<T> {
        MaybeUninit::new(data.assume_init_ref().clone())
    }

    #[inline]
    unsafe fn do_debug<T: std::fmt::Debug>(
        data: &MaybeUninit<T>,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Occupied: {:?}", data.assume_init_ref())
    }
}

impl MaybeAvailable for Vacancy {
    #[inline]
    unsafe fn do_maybe_ref<T>(_data: &MaybeUninit<T>) -> Option<&T> {
        None
    }
    #[inline]
    unsafe fn do_maybe_mut<T>(_data: &mut MaybeUninit<T>) -> Option<&mut T> {
        None
    }
    #[inline]
    unsafe fn do_set<T>(data: &mut MaybeUninit<T>, value: T) {
        *data = MaybeUninit::new(value)
    }
    #[inline]
    unsafe fn do_drop<T>(_data: &mut MaybeUninit<T>) {}
    #[inline]
    unsafe fn do_clone<T: Clone>(_data: &MaybeUninit<T>) -> MaybeUninit<T> {
        MaybeUninit::uninit()
    }
    #[inline]
    unsafe fn do_debug<T: std::fmt::Debug>(
        _data: &MaybeUninit<T>,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "Vacancy")
    }
}
