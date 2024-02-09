/// The KHash library is intended as a functional clone of the khash library in htslib, to the extent
/// that hash tables and sets created in C can be safely read and manipulated in Rust and vice versa
/// (with the caveat that interoperability requires the use of C types for keys and values).  
///
/// In particular, the C library does not do anything to deallocate keys or values, so using kh_destroy() for
/// example from htslib on a KHash created table could result in memory loss or double deallocation of the main
/// table structures.
///
/// Outside of that, when working with simple keys and values that do not require special deallocation, a KHash table
/// created in Rust can be passed to C and used to lookup, add and delete values without issues, as the main table
/// structures are all allocated using libc::malloc using the same logic as the htslib khash library.
///
/// While the internal behaviour of KHash and khash are as close as possible, the API of KHash is modelled on
/// [std::collections::HashMap] and [std::collections::HashSet], with safe methods for inserting, deleting, checking
/// and iterating over the tables and sets.
pub mod khash;
pub mod khash_error;
pub mod khash_func;
pub mod khash_map;
pub mod khash_set;

pub use khash::*;
pub use khash_func::*;
pub use khash_map::*;
pub use khash_set::*;
