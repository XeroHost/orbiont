#[cfg(test)]
mod tests {
    use glib::ToVariant;

    fn strings() -> Vec<String> {
        vec![
            String::from("first"),
            String::new(),
            String::from("tercero — 世界"),
            "long final string ".repeat(128),
        ]
    }

    #[test]
    fn next_reads_owned_strings_and_exhausts() {
        let expected = strings();
        let variant = expected.to_variant();
        let mut iter = variant.array_iter_str().unwrap();
        for (index, value) in expected.iter().enumerate() {
            assert_eq!(iter.len(), expected.len() - index);
            assert_eq!(iter.next(), Some(value.as_str()));
        }
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn next_back_reads_owned_strings() {
        let expected = strings();
        let variant = expected.to_variant();
        assert_eq!(
            variant.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
            expected
                .iter()
                .rev()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn nth_and_nth_back_read_owned_strings() {
        let expected = strings();
        let variant = expected.to_variant();
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.nth(1), Some(expected[1].as_str()));
        assert_eq!(iter.nth_back(0), Some(expected[3].as_str()));
        assert_eq!(iter.next_back(), Some(expected[2].as_str()));
        assert_eq!(iter.len(), 0);
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.nth_back(1), Some(expected[2].as_str()));
        assert_eq!(iter.nth(0), Some(expected[0].as_str()));
        assert_eq!(iter.last(), Some(expected[1].as_str()));
    }

    #[test]
    fn last_reads_owned_string_after_front_consumption() {
        let expected = strings();
        let variant = expected.to_variant();
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.next(), Some(expected[0].as_str()));
        assert_eq!(iter.last(), Some(expected[3].as_str()));
    }

    #[test]
    fn empty_and_overflowing_skips_remain_exhausted() {
        let empty = Vec::<String>::new().to_variant();
        assert_eq!(empty.array_iter_str().unwrap().last(), None);
        let variant = strings().to_variant();
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.nth(usize::MAX), None);
        assert_eq!(iter.next_back(), None);
        let mut iter = variant.array_iter_str().unwrap();
        assert_eq!(iter.nth_back(usize::MAX), None);
        assert_eq!(iter.next(), None);
    }
}
