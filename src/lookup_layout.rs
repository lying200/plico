//! Completion lookup placement in logical pixels. Mirrors LookupUi's preference
//! for below the input line, flipping only when above provides more room.
#[derive(Debug, PartialEq)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub fn place(line: [f32; 3], desired: [f32; 2], viewport: [f32; 2]) -> Placement {
    let [left, top, bottom] = line;
    let [view_width, view_height] = viewport;
    let margin = 8.0_f32.min(view_width / 2.).min(view_height / 2.).max(0.);
    let right = (view_width - margin).max(margin);
    let floor = (view_height - margin).max(margin);
    let top = top.clamp(margin, floor);
    let bottom = bottom.clamp(top, floor);
    let above = top - margin;
    let below = floor - bottom;
    let flip = desired[1] > below && above > below;
    let height = desired[1].max(0.).min(if flip { above } else { below });
    let width = desired[0].max(0.).min(right - margin);
    Placement {
        x: left.clamp(margin, right - width),
        y: if flip { top - height } else { bottom },
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flips_without_covering_the_input_line_and_clamps_horizontal_edges() {
        let p = place([460., 350., 370.], [200., 218.], [500., 400.]);
        assert_eq!(
            p,
            Placement {
                x: 292.,
                y: 132.,
                width: 200.,
                height: 218.
            }
        );
        assert!(p.y + p.height <= 350.);
        let p = place([20., 50., 70.], [200., 218.], [500., 400.]);
        assert_eq!(p.y, 70.);
        assert_eq!(p.height, 218.);
    }

    #[test]
    fn crops_to_the_larger_side_instead_of_covering_the_input_line() {
        let p = place([-10., 160., 180.], [200., 218.], [150., 300.]);
        assert_eq!(
            p,
            Placement {
                x: 8.,
                y: 8.,
                width: 134.,
                height: 152.
            }
        );
        let p = place([10., 90., 110.], [200., 218.], [500., 300.]);
        assert_eq!(p.y, 110.);
        assert_eq!(p.height, 182.);
        let p = place([0., 0., 0.], [200., 218.], [0., 0.]);
        assert_eq!(
            p,
            Placement {
                x: 0.,
                y: 0.,
                width: 0.,
                height: 0.
            }
        );
    }
}
