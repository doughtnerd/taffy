//! `calc()` values with `TaffyTree`: expressions stored in the tree and resolved during layout.

// NOTE: This example requires the `calc` feature flag to be enabled (it is by default).

#[cfg(not(feature = "calc"))]
fn main() {
    println!("Error: this example requires the 'calc' feature to be enabled");
    println!("Try:");
    println!("    cargo run --example calc --features calc")
}

#[cfg(feature = "calc")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use taffy::prelude::*;
    use taffy::{Calc, CalcFunction};

    let mut taffy: TaffyTree<()> = TaffyTree::new();

    // width: calc(50% - 12px). `new_calc` stores the expression in the tree and returns the style value that refers to it.
    let half_minus_gutter: Dimension = taffy.new_calc(Calc::Sum(vec![Calc::Percent(0.5), Calc::Length(-12.0)]))?;

    // height: clamp(20px, 10%, 40px)
    let clamped: Dimension = taffy.new_calc(Calc::Function(
        CalcFunction::Clamp,
        vec![Calc::Length(20.0), Calc::Percent(0.1), Calc::Length(40.0)],
    ))?;

    // padding-left and -right: calc(2% + 4px)
    let padding: LengthPercentage = taffy.new_calc(Calc::Sum(vec![Calc::Percent(0.02), Calc::Length(4.0)]))?;

    // A percentage-free expression simplifies to a plain length, as CSS does when it parses one: calc(10px * 2) is 20px
    let border: LengthPercentage = taffy.new_calc(Calc::Product(vec![Calc::Length(10.0), Calc::Number(2.0)]))?;
    assert_eq!(border, LengthPercentage::length(20.0));

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
