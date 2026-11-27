//! `calc()` values with `TaffyTree`: built as `Calc` expressions, or parsed from CSS with the `parse` feature.
use taffy::prelude::*;
use taffy::{Calc, CalcFunction};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut taffy: TaffyTree<()> = TaffyTree::new();

    // width: calc(50% - 12px), built as an expression. `calc_value` stores it in the tree and returns the style value
    // that refers to it.
    let half_minus_gutter: Dimension = taffy.calc_value(Calc::Sum(vec![Calc::Percent(0.5), Calc::Length(-12.0)]))?;

    // height: clamp(20px, 10%, 40px)
    let clamped: Dimension = taffy.calc_value(Calc::Function(
        CalcFunction::Clamp,
        vec![Calc::Length(20.0), Calc::Percent(0.1), Calc::Length(40.0)],
    ))?;

    // padding-left and -right: calc(2% + 4px), parsed from CSS (`parse` feature). A percentage-free expression such as
    // `calc(10px * 2)` simplifies to a plain length, as CSS does when it parses it.
    #[cfg(feature = "parse")]
    let padding: LengthPercentage = taffy.parse_value("calc(2% + 4px)")?;
    #[cfg(not(feature = "parse"))]
    let padding: LengthPercentage = taffy.calc_value(Calc::Sum(vec![Calc::Percent(0.02), Calc::Length(4.0)]))?;

    let child = taffy.new_leaf(Style {
        size: Size { width: half_minus_gutter, height: clamped },
        padding: Rect { left: padding, right: padding, top: zero(), bottom: zero() },
        ..Default::default()
    })?;
    let root = taffy.new_with_children(
        Style { size: Size { width: length(600.0), height: length(300.0) }, ..Default::default() },
        &[child],
    )?;

    taffy.compute_layout(root, Size::MAX_CONTENT)?;
    // width 50% of 600 - 12 = 288; height clamp(20, 30, 40) = 30; padding 2% of 600 + 4 = 16
    let layout = taffy.layout(child)?;
    println!("child: {:?}, padding {:?}", layout.size, layout.padding);
    assert_eq!(layout.size, Size { width: 288.0, height: 30.0 });
    assert_eq!(layout.padding.left, 16.0);
    Ok(())
}
