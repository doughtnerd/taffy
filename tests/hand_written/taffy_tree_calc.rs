//! `TaffyTree`'s `calc()` storage: what generated tests can't express. Layout results are Chrome 158's.
#[cfg(feature = "calc")]
mod taffy_tree_calc {
    use taffy::prelude::*;
    use taffy::{BoxSizing, Calc};
    use taffy_test_helpers::{TestNodeContext, test_measure_function_with_calc};

    /// `calc(percent% + px)`
    fn lin(percent: f32, px: f32) -> Calc {
        Calc::Sum(vec![Calc::Percent(percent / 100.0), Calc::Length(px)])
    }

    fn sized(width: Dimension, height: Dimension) -> Style {
        Style { size: Size { width, height }, ..Default::default() }
    }

    fn rect<T>(tree: &TaffyTree<T>, node: NodeId) -> [f32; 4] {
        let l = tree.layout(node).unwrap();
        [l.location.x, l.location.y, l.size.width, l.size.height]
    }

    #[test]
    fn calc_resolves_against_each_propertys_basis() {
        // width: calc(50% - 12px); margin-left: calc(10% - 10px); padding-left: calc(5% + 2px); column-gap: calc(5% - 5px)
        let mut t: TaffyTree<()> = TaffyTree::new();
        let width = t.new_calc(lin(50.0, -12.0)).unwrap();
        let margin = t.new_calc(lin(10.0, -10.0)).unwrap();
        let padding = t.new_calc(lin(5.0, 2.0)).unwrap();
        let gap = t.new_calc(lin(5.0, -5.0)).unwrap();
        let a =
            t.new_leaf(Style { margin: Rect { left: margin, ..Rect::zero() }, ..sized(width, length(10.0)) }).unwrap();
        let b = t.new_leaf(sized(length(20.0), length(10.0))).unwrap();
        let row = t
            .new_with_children(
                Style {
                    padding: Rect { left: padding, ..Rect::zero() },
                    gap: Size { width: gap, height: zero() },
                    align_items: AlignItems::FLEX_START,
                    ..sized(percent(1.0), length(100.0))
                },
                &[a, b],
            )
            .unwrap();
        let root = t.new_with_children(sized(length(400.0), length(100.0)), &[row]).unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        // padding-left is 5% of 400 + 2 = 22, so the row's content box is 378 wide. a is 50% of 378 - 12 = 177 wide
        // after a 37.8 - 10 = 27.8 margin, and b follows a 18.9 - 5 = 13.9 gap (rounded to whole pixels).
        assert_eq!(rect(&t, a), [50.0, 0.0, 177.0, 10.0]);
        assert_eq!(rect(&t, b), [241.0, 0.0, 20.0, 10.0]);
    }

    #[test]
    fn new_calc_simplifies_a_percentage_free_expression() {
        // As a plain 15px, height: calc(10px + 5px) is definite in an auto-height column; calc(50% + 10px) is auto
        let mut t: TaffyTree<()> = TaffyTree::new();
        let fixed: Dimension = t.new_calc(Calc::Sum(vec![Calc::Length(10.0), Calc::Length(5.0)])).unwrap();
        assert_eq!(fixed, Dimension::length(15.0));
        let half: Dimension = t.new_calc(lin(50.0, 10.0)).unwrap();
        let inner = t.new_leaf(sized(auto(), length(30.0))).unwrap();
        let a = t.new_with_children(sized(length(20.0), half), &[inner]).unwrap();
        let b = t.new_leaf(sized(length(20.0), fixed)).unwrap();
        let root = t
            .new_with_children(
                Style {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FLEX_START,
                    ..sized(length(400.0), auto())
                },
                &[a, b],
            )
            .unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(rect(&t, a)[3], 30.0);
        assert_eq!(rect(&t, b), [0.0, 30.0, 20.0, 15.0]);
    }

    #[test]
    fn a_measure_function_resolves_calc_in_its_leafs_style() {
        // padding-left: calc(10% + 2px); padding-top: calc(5% - 4px); a 50 x 20 content box: 92 x 36
        let mut t: TaffyTree<TestNodeContext> = TaffyTree::new();
        let left = t.new_calc(lin(10.0, 2.0)).unwrap();
        let top = t.new_calc(lin(5.0, -4.0)).unwrap();
        let style = Style {
            box_sizing: BoxSizing::ContentBox,
            padding: Rect { left, top, right: zero(), bottom: zero() },
            ..sized(length(50.0), length(20.0))
        };
        let leaf = t.new_leaf_with_context(style, TestNodeContext::fixed(50.0, 20.0)).unwrap();
        let root_style = Style { align_items: AlignItems::FLEX_START, ..sized(length(400.0), length(300.0)) };
        let root = t.new_with_children(root_style, &[leaf]).unwrap();
        t.compute_layout_with_measure_and_calc(root, Size::MAX_CONTENT, test_measure_function_with_calc).unwrap();
        assert_eq!(rect(&t, leaf)[2..], [92.0, 36.0]);
    }

    #[test]
    fn expressions_are_shared_removed_and_cleared() {
        let mut t: TaffyTree<()> = TaffyTree::new();
        let width: Dimension = t.new_calc(lin(50.0, 0.0)).unwrap();
        let handle = width.into_raw().calc_value();
        let a = t.new_leaf(sized(width, length(10.0))).unwrap();
        let b = t.new_leaf(sized(width, length(10.0))).unwrap();
        let root_style = Style { flex_direction: FlexDirection::Column, ..sized(length(200.0), length(100.0)) };
        let root = t.new_with_children(root_style, &[a, b]).unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!((rect(&t, a)[2], rect(&t, b)[2]), (100.0, 100.0));
        assert_eq!(t.calc(handle), Some(&lin(50.0, 0.0)));

        // A style that still refers to a removed expression resolves it to 0
        assert_eq!(t.remove_calc(handle), Some(lin(50.0, 0.0)));
        assert_eq!(t.remove_calc(handle), None);
        t.mark_dirty(a).unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(rect(&t, a)[2], 0.0);

        // A new expression reusing the slot doesn't answer to the old handle
        let reused: Dimension = t.new_calc(lin(25.0, 1.0)).unwrap();
        assert_eq!(t.calc(handle), None);
        t.clear();
        assert_eq!(t.calc(reused.into_raw().calc_value()), None);
    }

    #[test]
    fn expressions_that_are_not_lengths_are_refused() {
        let mut t: TaffyTree<()> = TaffyTree::new();
        assert!(t.new_calc::<Dimension>(Calc::Number(2.0)).is_err());
        assert!(t.new_calc::<Dimension>(Calc::Sum(vec![Calc::Length(1.0), Calc::Number(2.0)])).is_err());
        assert!(t.new_calc::<Dimension>(Calc::Product(vec![Calc::Length(1.0), Calc::Length(2.0)])).is_err());
    }
}
