//! Public facade for `PliegoCSS`.
//!
//! The public API validates utilities during Rust compilation and exposes their emitted class
//! identity to Rust renderers.
//!
//! The supported candidate surface is [`pc!`], [`pcx!`], [`Style`], and [`StyleId`]. Internal
//! compiler crates remain an exact-version implementation graph rather than application APIs.
//!
//! ```
//! use pliego_css::{Style, pc, pcx};
//!
//! const CARD: Style = pc!("flex gap-4 rounded-lg");
//!
//! let active = true;
//! let selected = pcx!(
//!     "block",
//!     if active { "opacity-100" } else { "opacity-50" },
//! );
//!
//! assert!(!CARD.is_empty());
//! assert!(selected.class_name().starts_with("pc_"));
//! assert_eq!(Style::EMPTY.class_name(), "");
//! ```

#![forbid(unsafe_code)]

extern crate self as pliego_css;

use core::fmt;

/// Implementation exports used by the hygienic facade macros.
#[doc(hidden)]
pub mod __private {
    pub use pliego_css_macros::{pc_id, pcx_id};
}

/// Validates one utility string at compile time and returns its static [`Style`].
///
/// The declarative wrapper keeps `$crate` hygiene when the Cargo dependency is renamed; semantic
/// parsing and identity derivation are delegated to the exact-version procedural macro crate.
#[macro_export]
macro_rules! pc {
    ($($tokens:tt)*) => {
        $crate::Style::__from_compiled_id($crate::__private::pc_id!($($tokens)*))
    };
}

/// Compiles every visible combination of conditional utility strings and selects one [`Style`].
///
/// Selector expressions are evaluated once in source order by the internal procedural expansion.
#[macro_export]
macro_rules! pcx {
    ($($tokens:tt)*) => {
        $crate::Style::__from_compiled_id($crate::__private::pcx_id!($($tokens)*))
    };
}

/// A compact identity for one normalized, theme-scoped style.
///
/// Application code receives IDs from [`Style::id`]. Integer construction is not part of the
/// supported application API; the doc-hidden hook on [`Style`] exists only for macro expansion.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct StyleId(pliego_css_ir::StyleId);

impl StyleId {
    /// Returns the complete deterministic 128-bit representation.
    #[must_use]
    pub const fn get(self) -> u128 {
        self.0.get()
    }

    /// Returns whether this is the empty style sentinel.
    #[must_use]
    pub const fn is_unresolved(self) -> bool {
        self.0.is_unresolved()
    }

    /// Encodes all identity bits as a lowercase CSS-safe class name.
    ///
    /// The empty sentinel encodes mechanically as `pc_0`; use [`Style::class_name`]
    /// when rendering a [`Style`], because [`Style::EMPTY`] intentionally renders
    /// no class at all.
    #[must_use]
    pub fn to_class_name(self) -> String {
        self.0.to_class_name()
    }
}

/// A compile-time validated style literal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct Style {
    id: StyleId,
}

impl Style {
    /// The empty style handle for conditional composition.
    pub const EMPTY: Self = Self {
        id: StyleId(pliego_css_ir::StyleId::UNRESOLVED),
    };

    /// Creates a style identity already validated by the matching PliegoCSS macro crate.
    #[doc(hidden)]
    #[must_use]
    pub const fn __from_compiled_id(id: u128) -> Self {
        Self {
            id: StyleId(pliego_css_ir::StyleId::new(id)),
        }
    }

    /// Returns the stable identity derived from normalized semantics.
    #[must_use]
    pub const fn id(self) -> StyleId {
        self.id
    }

    /// Returns the CSS class emitted for this style.
    ///
    /// Empty conditional styles return an empty string and can be interpolated directly.
    #[must_use]
    pub fn class_name(self) -> String {
        if self.is_empty() {
            String::new()
        } else {
            self.id.to_class_name()
        }
    }

    /// Returns `true` when the handle contains no utilities.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.id.is_unresolved()
    }
}

impl fmt::Display for Style {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            Ok(())
        } else {
            formatter.write_str(&self.id.to_class_name())
        }
    }
}

impl From<Style> for String {
    fn from(style: Style) -> Self {
        style.class_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REUSABLE_CARD: Style = pc!("rounded-lg bg-surface p-6");

    #[test]
    fn macro_returns_validated_static_style() {
        let style = pc!("flex items-center gap-4");
        assert!(!style.id().is_unresolved());
        assert!(style.class_name().starts_with("pc_"));
        assert!(!style.is_empty());
    }

    #[test]
    fn empty_style_is_explicit() {
        assert!(Style::EMPTY.is_empty());
        assert!(Style::EMPTY.id().is_unresolved());
        assert_eq!(Style::EMPTY.id().to_class_name(), "pc_0");
        assert!(Style::EMPTY.class_name().is_empty());
        assert_eq!(Style::EMPTY.to_string(), "");
    }

    #[test]
    fn style_formats_as_its_css_class() {
        let style = pc!("flex gap-4");

        assert_eq!(style.to_string(), style.class_name());
        assert_eq!(String::from(style), style.class_name());
    }

    #[test]
    fn runtime_handle_is_only_the_style_identity() {
        assert_eq!(
            core::mem::size_of::<StyleId>(),
            core::mem::size_of::<u128>()
        );
        assert_eq!(
            core::mem::size_of::<Style>(),
            core::mem::size_of::<StyleId>()
        );
    }

    #[test]
    fn validated_styles_are_reusable_constants() {
        assert_eq!(REUSABLE_CARD.id(), pc!("p-6 bg-surface rounded-lg").id());
    }

    #[test]
    fn equivalent_utility_order_has_one_identity() {
        assert_eq!(pc!("flex gap-4").id(), pc!("gap-4 flex").id());
    }

    #[test]
    fn conditional_if_returns_a_complete_precompiled_style() {
        let selected = true;
        let style = pcx!(
            "inline-flex rounded-md bg-surface",
            if selected {
                "bg-accent text-white"
            } else {
                "text-ink"
            },
        );

        assert_eq!(
            style.id(),
            pc!("inline-flex rounded-md bg-accent text-white").id()
        );
    }

    #[test]
    fn conditional_match_compiles_every_literal_arm() {
        enum Variant {
            Primary,
            Ghost,
        }

        let variant = Variant::Ghost;
        let style = pcx!(
            "rounded-md",
            match variant {
                Variant::Primary => "bg-accent text-white",
                Variant::Ghost => "bg-transparent text-accent",
            },
        );

        assert_eq!(
            style.id(),
            pc!("rounded-md bg-transparent text-accent").id()
        );
        let _ = Variant::Primary;
    }

    #[test]
    fn independent_clauses_select_one_precompiled_cartesian_result() {
        let active = false;
        let compact = true;
        let style = pcx!(
            "inline-flex rounded-md",
            if active { "opacity-100" } else { "opacity-50" },
            if compact { "text-sm" } else { "text-lg" },
        );

        assert_eq!(
            style.id(),
            pc!("inline-flex rounded-md opacity-50 text-sm").id()
        );
    }

    #[test]
    fn each_independent_clause_expression_is_evaluated_once() {
        use core::cell::Cell;

        let evaluations = Cell::new(0_u8);
        let condition = || {
            evaluations.set(evaluations.get() + 1);
            true
        };
        let style = pcx!(
            "block",
            if condition() {
                "opacity-100"
            } else {
                "opacity-50"
            },
            if condition() { "text-sm" } else { "text-lg" },
        );

        assert_eq!(evaluations.get(), 2);
        assert_eq!(style.id(), pc!("block opacity-100 text-sm").id());
    }

    #[test]
    fn independent_if_and_match_preserve_rust_selection_semantics() {
        enum Size {
            Small,
            Large,
        }

        let active = true;
        let size = Size::Large;
        let style = pcx!(
            "inline-flex",
            if active { "opacity-100" } else { "opacity-50" },
            match size {
                Size::Small => "text-sm",
                Size::Large => "text-lg",
            },
        );

        assert_eq!(style.id(), pc!("inline-flex opacity-100 text-lg").id());
        let _ = Size::Small;
    }
}
