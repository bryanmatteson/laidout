//! Associative line-shape measurements for bottom-up layout experiments.
//!
//! A fragment is summarized by its first, last, and widest line plus its
//! height. Concatenation merges the left fragment's last line with the right
//! fragment's first line. Unlike the prototype's `Measurement.Add`, this
//! operation is a lawful monoid and therefore safe to use when reassociating
//! a document tree during bottom-up frontier construction.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Measurement {
    pub height: u32,
    pub max_width: u32,
    pub first_line: u32,
    pub last_line: u32,
}

impl Default for Measurement {
    fn default() -> Self {
        Self::empty()
    }
}

impl Measurement {
    /// The identity fragment: one empty line.
    pub const fn empty() -> Self {
        Self {
            height: 1,
            max_width: 0,
            first_line: 0,
            last_line: 0,
        }
    }

    /// A single unbroken text run.
    pub const fn text(width: u32) -> Self {
        Self {
            height: 1,
            max_width: width,
            first_line: width,
            last_line: width,
        }
    }

    /// Construct a measurement from one or more rendered line widths.
    pub fn from_line_widths(widths: &[u32]) -> Self {
        assert!(
            !widths.is_empty(),
            "a rendering always contains at least one line"
        );
        Self {
            height: widths.len().try_into().expect("line count exceeds u32"),
            max_width: *widths.iter().max().unwrap(),
            first_line: widths[0],
            last_line: widths[widths.len() - 1],
        }
    }

    pub const fn has_break(self) -> bool {
        self.height > 1
    }

    /// Associative fragment concatenation.
    pub fn append(self, other: Self) -> Self {
        let seam = self
            .last_line
            .checked_add(other.first_line)
            .expect("line width exceeds u32");
        Self {
            height: self
                .height
                .checked_add(other.height)
                .and_then(|height| height.checked_sub(1))
                .expect("line count exceeds u32"),
            max_width: self.max_width.max(other.max_width).max(seam),
            first_line: if self.height == 1 {
                seam
            } else {
                self.first_line
            },
            last_line: if other.height == 1 {
                self.last_line
                    .checked_add(other.last_line)
                    .expect("line width exceeds u32")
            } else {
                other.last_line
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Measurement;

    #[test]
    fn empty_is_a_two_sided_identity() {
        let fragment = Measurement::from_line_widths(&[3, 8, 2]);
        assert_eq!(Measurement::empty().append(fragment), fragment);
        assert_eq!(fragment.append(Measurement::empty()), fragment);
    }

    #[test]
    fn append_merges_only_the_seam_lines() {
        let left = Measurement::from_line_widths(&[2, 5]);
        let right = Measurement::from_line_widths(&[4, 7]);
        assert_eq!(
            left.append(right),
            Measurement {
                height: 3,
                max_width: 9,
                first_line: 2,
                last_line: 7
            }
        );
    }

    #[test]
    fn broken_text_broken_counterexample_is_associative() {
        // The Go prototype's Add law returns different last-line widths for
        // these two associations. A lawful summary must describe the same
        // final lines [2, 12, 7] either way.
        let a = Measurement::from_line_widths(&[2, 5]);
        let b = Measurement::text(3);
        let c = Measurement::from_line_widths(&[4, 7]);
        let expected = Measurement::from_line_widths(&[2, 12, 7]);
        assert_eq!(a.append(b).append(c), expected);
        assert_eq!(a.append(b.append(c)), expected);
    }
}
