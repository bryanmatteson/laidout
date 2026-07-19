use std::num::NonZeroU32;

use laidout::{render, Doc, LayoutStrategy, RenderOptions};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    #[test]
    fn grouped_word_sequences_preserve_content(
        words in prop::collection::vec("[a-z]{1,8}", 0..20),
        width in 1u32..80,
    ) {
        let doc = Doc::<u32>::group(Doc::join(Doc::line(), words.iter().map(Doc::text)));
        for strategy in [LayoutStrategy::Fast, LayoutStrategy::Exact] {
            let rendered = render(
                &doc,
                RenderOptions::new(NonZeroU32::new(width).unwrap()).with_strategy(strategy),
            ).unwrap();
            prop_assert_eq!(
                rendered.text().split_whitespace().collect::<Vec<_>>(),
                words.iter().map(String::as_str).collect::<Vec<_>>(),
            );
        }
    }
}
