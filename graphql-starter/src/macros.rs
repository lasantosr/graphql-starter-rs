/// Creates a newtype struct implementing [From], [Deref](std::ops::Deref) and [DerefMut](std::ops::DerefMut)
#[macro_export]
macro_rules! newtype {
    ($(#[$attr:meta])* $new_v:vis $new_ty:ident $(< $($gen:ident$(: $gen_ty:path)?),* >)? ($v:vis $ty:path)) => {
        $(#[$attr])*
        $new_v struct $new_ty$(<$($gen$(: $gen_ty)?),*>)?($v $ty);
        $crate::newtype_impl!($new_ty$(<$($gen$(: $gen_ty)?),*>)?($ty));
    };
}

/// Implements [From], [Deref](std::ops::Deref) and [DerefMut](std::ops::DerefMut) for a newtype
#[macro_export]
macro_rules! newtype_impl {
    ($new_ty:ident $(< $($gen:ident$(: $gen_ty:path)?),* >)? ($ty:path)) => {
        impl$(<$($gen$(: $gen_ty)?),*>)? From<$ty> for $new_ty$(<$($gen),*>)? {
            fn from(o: $ty) -> Self {
                $new_ty(o)
            }
        }
        impl$(<$($gen$(: $gen_ty)?),*>)? From<$new_ty$(<$($gen),*>)?> for $ty {
            fn from(o: $new_ty$(<$($gen),*>)?) -> Self {
                o.0
            }
        }
        impl$(<$($gen$(: $gen_ty)?),*>)? ::std::ops::Deref for $new_ty$(<$($gen),*>)? {
            type Target = $ty;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl$(<$($gen$(: $gen_ty)?),*>)? ::std::ops::DerefMut for $new_ty$(<$($gen),*>)? {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}

/// Declares a `mod` and uses it.
///
/// ## Examples
/// ``` ignore
/// using!{
///   pub mod1,
///   pub(crate) mod2,
///   mod3
/// }
/// ```
/// Expands to:
/// ``` ignore
/// mod mod1;
/// pub use self::mod1::*;
/// mod mod2;
/// pub(crate) use self::mod2::*;
/// mod mod3;
/// use self::name::*;
/// ```
#[macro_export]
macro_rules! using {
    ($($v:vis $p:ident),*) => {
        $(
            mod $p;
            $v use self::$p::*;
        )*
    }
}
