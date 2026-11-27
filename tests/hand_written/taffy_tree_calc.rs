//! `calc()` values in a `TaffyTree`: the tree stores the expressions (`TaffyTree::new_calc`) and resolves their handles
//! during layout, for every property that accepts one. The expected boxes are Chrome's (158) for the same styles,
//! within its 1/64 px layout unit.
#[cfg(feature = "calc")]
mod taffy_tree_calc {
    use taffy::prelude::*;
    use taffy::{BoxSizing, Calc, CalcFunction, CalcHandle};

    /// Chrome lays out in 1/64 px units; Taffy in f32.
    const TOLERANCE: f32 = 1.0 / 32.0;

    fn tree() -> TaffyTree<()> {
        let mut tree = TaffyTree::new();
        tree.disable_rounding();
        tree
    }

    /// `calc(percent% + px)`
    fn lin(tree: &mut TaffyTree<()>, percent: f32, px: f32) -> CalcHandle {
        tree.new_calc(Calc::Sum(vec![Calc::Percent(percent / 100.0), Calc::Length(px)])).unwrap()
    }

    fn assert_box(tree: &TaffyTree<()>, node: NodeId, expected: [f32; 4]) {
        let l = tree.layout(node).unwrap();
        let actual = [l.location.x, l.location.y, l.size.width, l.size.height];
        assert!(
            actual.iter().zip(expected).all(|(a, e)| (a - e).abs() <= TOLERANCE),
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn sized(width: Dimension, height: Dimension) -> Style {
        Style { size: Size { width, height }, ..Default::default() }
    }

    #[test]
    fn sizes_and_comparison_functions() {
        let mut t = tree();
        let w = lin(&mut t, 50.0, -12.0);
        let h = lin(&mut t, 100.0, -20.0);
        // min(25%, 60px) and clamp(10px, 10%, 20px)
        let min = t.new_calc(Calc::Function(CalcFunction::Min, vec![Calc::Percent(0.25), Calc::Length(60.0)])).unwrap();
        let clamp = t
            .new_calc(Calc::Function(
                CalcFunction::Clamp,
                vec![Calc::Length(10.0), Calc::Percent(0.1), Calc::Length(20.0)],
            ))
            .unwrap();
        let a = t.new_leaf(sized(Dimension::calc(w), Dimension::calc(h))).unwrap();
        let b = t.new_leaf(sized(Dimension::calc(min), Dimension::calc(clamp))).unwrap();
        let root = t
            .new_with_children(
                Style { align_items: AlignItems::FLEX_START, ..sized(length(400.0), length(300.0)) },
                &[a, b],
            )
            .unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, a, [0.0, 0.0, 188.0, 280.0]);
        assert_box(&t, b, [188.0, 0.0, 60.0, 20.0]);
    }

    #[test]
    fn min_and_max_sizes() {
        let mut t = tree();
        let max = lin(&mut t, 25.0, 10.0);
        let min = lin(&mut t, 50.0, 10.0);
        let grow = |t: &mut TaffyTree<()>, extra: Style| {
            t.new_leaf(Style { flex_grow: 1.0, size: Size { width: auto(), height: length(10.0) }, ..extra }).unwrap()
        };
        let a = grow(
            &mut t,
            Style { max_size: Size { width: LengthPercentageAuto::calc(max), height: auto() }, ..Default::default() },
        );
        let b = grow(
            &mut t,
            Style { min_size: Size { width: LengthPercentageAuto::calc(min), height: auto() }, ..Default::default() },
        );
        let c = grow(&mut t, Style::default());
        let root = t.new_with_children(sized(length(400.0), length(100.0)), &[a, b, c]).unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, a, [0.0, 0.0, 95.0, 10.0]);
        assert_box(&t, b, [95.0, 0.0, 210.0, 10.0]);
        assert_box(&t, c, [305.0, 0.0, 95.0, 10.0]);
    }

    #[test]
    fn padding_and_margins_resolve_against_the_containing_block_width() {
        let mut t = tree();
        let pad = lin(&mut t, 5.0, 2.0);
        let ml = lin(&mut t, 10.0, -10.0);
        let mt = lin(&mut t, 5.0, 6.0);
        let child = t
            .new_leaf(Style {
                margin: Rect {
                    left: LengthPercentageAuto::calc(ml),
                    top: LengthPercentageAuto::calc(mt),
                    right: zero(),
                    bottom: zero(),
                },
                ..sized(length(100.0), length(40.0))
            })
            .unwrap();
        let container = t
            .new_with_children(
                Style {
                    align_items: AlignItems::FLEX_START,
                    padding: Rect {
                        left: LengthPercentage::calc(pad),
                        right: LengthPercentage::calc(pad),
                        top: LengthPercentage::calc(pad),
                        bottom: LengthPercentage::calc(pad),
                    },
                    ..sized(length(400.0), length(300.0))
                },
                &[child],
            )
            .unwrap();
        let root = t.new_with_children(sized(length(400.0), length(300.0)), &[container]).unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, child, [47.594, 45.797, 100.0, 40.0]);
    }

    #[test]
    fn percentage_borders() {
        // CSS has no percentage border widths, so no Chrome reference: calc(1% + 1px) of a 400px containing block is 5px.
        let mut t = tree();
        let b = lin(&mut t, 1.0, 1.0);
        let child = t.new_leaf(sized(length(10.0), length(10.0))).unwrap();
        let container = t
            .new_with_children(
                Style {
                    align_items: AlignItems::FLEX_START,
                    border: Rect {
                        left: LengthPercentage::calc(b),
                        top: LengthPercentage::calc(b),
                        right: zero(),
                        bottom: zero(),
                    },
                    ..sized(length(100.0), length(100.0))
                },
                &[child],
            )
            .unwrap();
        let root = t.new_with_children(sized(length(400.0), length(300.0)), &[container]).unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, child, [5.0, 5.0, 10.0, 10.0]);
    }

    #[test]
    fn gaps() {
        let mut t = tree();
        let column = lin(&mut t, 5.0, -5.0);
        let row = lin(&mut t, 10.0, -20.0);
        let kids: Vec<NodeId> = (0..3).map(|_| t.new_leaf(sized(length(150.0), length(20.0))).unwrap()).collect();
        let root = t
            .new_with_children(
                Style {
                    flex_wrap: FlexWrap::Wrap,
                    align_content: AlignContent::FLEX_START,
                    gap: Size { width: LengthPercentage::calc(column), height: LengthPercentage::calc(row) },
                    ..sized(length(400.0), length(300.0))
                },
                &kids,
            )
            .unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, kids[0], [0.0, 0.0, 150.0, 20.0]);
        assert_box(&t, kids[1], [165.0, 0.0, 150.0, 20.0]);
        assert_box(&t, kids[2], [0.0, 30.0, 150.0, 20.0]);
    }

    #[test]
    fn absolute_insets() {
        let mut t = tree();
        let left = lin(&mut t, 25.0, 4.0);
        let top = lin(&mut t, 50.0, -10.0);
        let right = lin(&mut t, 10.0, -20.0);
        let height = lin(&mut t, 10.0, 5.0);
        let abs = t
            .new_leaf(Style {
                position: Position::Absolute,
                inset: Rect {
                    left: LengthPercentageAuto::calc(left),
                    top: LengthPercentageAuto::calc(top),
                    right: LengthPercentageAuto::calc(right),
                    bottom: auto(),
                },
                size: Size { width: auto(), height: Dimension::calc(height) },
                ..Default::default()
            })
            .unwrap();
        let root = t
            .new_with_children(Style { position: Position::Relative, ..sized(length(400.0), length(300.0)) }, &[abs])
            .unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, abs, [104.0, 140.0, 276.0, 35.0]);
    }

    #[test]
    fn flex_basis() {
        let mut t = tree();
        let a_basis = lin(&mut t, 40.0, -20.0);
        let b_basis = lin(&mut t, 20.0, 10.0);
        let item = |t: &mut TaffyTree<()>, basis| {
            t.new_leaf(Style { flex_basis: Dimension::calc(basis), flex_shrink: 0.0, ..sized(auto(), length(20.0)) })
                .unwrap()
        };
        let a = item(&mut t, a_basis);
        let b = item(&mut t, b_basis);
        let root = t.new_with_children(sized(length(500.0), length(100.0)), &[a, b]).unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, a, [0.0, 0.0, 180.0, 20.0]);
        assert_box(&t, b, [180.0, 0.0, 110.0, 20.0]);
    }

    #[cfg(feature = "grid")]
    #[test]
    fn grid_tracks() {
        // grid-template-columns: calc(25% + 10px) minmax(calc(10% + 5px), 1fr) fit-content(calc(20% - 10px));
        // grid-template-rows: calc(50% - 20px)
        let mut t = tree();
        let fixed = lin(&mut t, 25.0, 10.0);
        let min = lin(&mut t, 10.0, 5.0);
        let limit = lin(&mut t, 20.0, -10.0);
        let row = lin(&mut t, 50.0, -20.0);
        let kids = [
            t.new_leaf(Style::default()).unwrap(),
            t.new_leaf(Style::default()).unwrap(),
            t.new_leaf(sized(length(200.0), auto())).unwrap(),
        ];
        let root = t
            .new_with_children(
                Style {
                    display: Display::Grid,
                    grid_template_columns: vec![
                        GridTemplateComponent::Single(TrackSizingFunction::from(LengthPercentage::calc(fixed))),
                        minmax(MinTrackSizingFunction::calc(min), MaxTrackSizingFunction::fr(1.0)),
                        GridTemplateComponent::Single(TrackSizingFunction {
                            min: MinTrackSizingFunction::AUTO,
                            max: MaxTrackSizingFunction::fit_content_calc(limit),
                        }),
                    ],
                    grid_template_rows: vec![GridTemplateComponent::Single(TrackSizingFunction::from(
                        LengthPercentage::calc(row),
                    ))],
                    ..sized(length(400.0), length(200.0))
                },
                &kids,
            )
            .unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, kids[0], [0.0, 0.0, 110.0, 80.0]);
        assert_box(&t, kids[1], [110.0, 0.0, 90.0, 80.0]);
        assert_box(&t, kids[2], [200.0, 0.0, 200.0, 80.0]);
    }

    #[cfg(feature = "block_layout")]
    #[test]
    fn block_layout_sizes_margins_and_fit_content_limits() {
        let mut t = tree();
        let w = lin(&mut t, 50.0, 10.0);
        let ml = lin(&mut t, 10.0, 5.0);
        let h = lin(&mut t, 20.0, -10.0);
        let max = lin(&mut t, 30.0, 1.0);
        let a = t
            .new_leaf(Style {
                display: Display::Block,
                margin: Rect { left: LengthPercentageAuto::calc(ml), right: zero(), top: zero(), bottom: zero() },
                ..sized(Dimension::calc(w), Dimension::calc(h))
            })
            .unwrap();
        let wide = t.new_leaf(Style { display: Display::Block, ..sized(length(300.0), length(10.0)) }).unwrap();
        let b = t
            .new_with_children(
                Style {
                    display: Display::Block,
                    max_size: Size { width: LengthPercentageAuto::calc(max), height: auto() },
                    ..sized(Dimension::fit_content(), length(10.0))
                },
                &[wide],
            )
            .unwrap();
        let root = t
            .new_with_children(Style { display: Display::Block, ..sized(length(400.0), length(200.0)) }, &[a, b])
            .unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, a, [45.0, 0.0, 210.0, 30.0]);
        assert_box(&t, b, [0.0, 30.0, 121.0, 10.0]);
    }

    #[test]
    fn a_percentage_against_an_indefinite_basis_behaves_like_one() {
        // height: calc(50% + 10px) in an auto-height column is auto, as height: 50% would be; calc(10px + 5px) is 15px.
        let mut t = tree();
        let half = lin(&mut t, 50.0, 10.0);
        // calc_value simplifies the percentage-free expression to 15px, as CSS does when it parses it.
        let fixed: Dimension = t.calc_value(Calc::Sum(vec![Calc::Length(10.0), Calc::Length(5.0)])).unwrap();
        let inner = t.new_leaf(sized(auto(), length(30.0))).unwrap();
        let a = t.new_with_children(sized(length(20.0), Dimension::calc(half)), &[inner]).unwrap();
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
        assert_box(&t, root, [0.0, 0.0, 400.0, 45.0]);
        assert_box(&t, a, [0.0, 0.0, 20.0, 30.0]);
        assert_box(&t, b, [0.0, 30.0, 20.0, 15.0]);
    }

    #[test]
    fn a_measured_leaf_resolves_calc_in_its_own_padding() {
        // padding-left: calc(10% + 2px); padding-top: calc(5% - 4px); 50 x 20 of content (content-box): Chrome's 92 x 36.
        use taffy_test_helpers::{TestNodeContext, test_measure_function_with_calc};
        let mut t: TaffyTree<TestNodeContext> = TaffyTree::new();
        t.disable_rounding();
        let left: LengthPercentage = t.calc_value(Calc::Sum(vec![Calc::Percent(0.1), Calc::Length(2.0)])).unwrap();
        let top: LengthPercentage = t.calc_value(Calc::Sum(vec![Calc::Percent(0.05), Calc::Length(-4.0)])).unwrap();
        let leaf = t
            .new_leaf_with_context(
                Style {
                    box_sizing: BoxSizing::ContentBox,
                    padding: Rect { left, top, right: zero(), bottom: zero() },
                    ..sized(length(50.0), length(20.0))
                },
                TestNodeContext::fixed(50.0, 20.0),
            )
            .unwrap();
        let root = t
            .new_with_children(
                Style { align_items: AlignItems::FLEX_START, ..sized(length(400.0), length(300.0)) },
                &[leaf],
            )
            .unwrap();
        t.compute_layout_with_measure_and_calc(root, Size::MAX_CONTENT, test_measure_function_with_calc).unwrap();
        let l = t.layout(leaf).unwrap();
        assert_eq!((l.size.width, l.size.height), (92.0, 36.0));

        // compute_layout (no measure function) resolves them too, around an empty leaf.
        let mut t = tree();
        let left: LengthPercentage = t.calc_value(Calc::Sum(vec![Calc::Percent(0.1), Calc::Length(2.0)])).unwrap();
        let leaf = t
            .new_leaf(Style {
                padding: Rect { left, top: zero(), right: zero(), bottom: zero() },
                ..Default::default()
            })
            .unwrap();
        let root = t
            .new_with_children(
                Style { align_items: AlignItems::FLEX_START, ..sized(length(400.0), length(300.0)) },
                &[leaf],
            )
            .unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, leaf, [0.0, 0.0, 42.0, 0.0]);
    }

    #[test]
    fn handles_can_be_shared_removed_and_cleared() {
        let mut t = tree();
        let w = lin(&mut t, 50.0, 0.0);
        let a = t.new_leaf(sized(Dimension::calc(w), length(10.0))).unwrap();
        let b = t.new_leaf(sized(Dimension::calc(w), length(10.0))).unwrap();
        let root = t
            .new_with_children(
                Style {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FLEX_START,
                    ..sized(length(200.0), auto())
                },
                &[a, b],
            )
            .unwrap();
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, a, [0.0, 0.0, 100.0, 10.0]);
        assert_box(&t, b, [0.0, 10.0, 100.0, 10.0]);
        assert_eq!(t.calc(w), Some(&Calc::Sum(vec![Calc::Percent(0.5), Calc::Length(0.0)])));

        // Replacing one style's expression: a new handle, the old one removed once nothing uses it.
        let w2 = lin(&mut t, 25.0, 0.0);
        t.set_style(a, sized(Dimension::calc(w2), length(10.0))).unwrap();
        t.set_style(b, sized(Dimension::calc(w2), length(10.0))).unwrap();
        assert!(t.remove_calc(w).is_some());
        assert!(t.remove_calc(w).is_none());
        t.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_box(&t, a, [0.0, 0.0, 50.0, 10.0]);

        // A handle the tree no longer holds resolves to 0.
        assert_eq!(t.calc(w), None);
        t.clear();
        assert_eq!(t.calc(w2), None);
    }

    #[test]
    fn expressions_that_are_not_lengths_are_refused() {
        let mut t = tree();
        assert!(t.new_calc(Calc::Number(2.0)).is_err());
        assert!(t.new_calc(Calc::Sum(vec![Calc::Length(1.0), Calc::Number(2.0)])).is_err());
        assert!(t.new_calc(Calc::Product(vec![Calc::Length(1.0), Calc::Length(2.0)])).is_err());
    }
}
