/// This file defines the Color trait and all of the standard color types that implement it.

use std::collections::HashMap;
use std::convert::From;
use std::num::ParseIntError;
use std::result::Result::Err;
use std::string::ToString;

use super::coord::Coord;
use illuminants::{Illuminant};

extern crate termion;
use self::termion::color::{Fg, Bg, Reset, Rgb};



/// A point in the CIE 1931 XYZ color space. Although any point in XYZ coordinate space is technically
/// valid, in this library XYZ colors are treated as normalized so that Y=1 is the white point of
/// whatever illuminant is being worked with.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct XYZColor {
    // these need to all be positive
    // TODO: way of implementing this constraint in code?
    /// The X axis of the CIE 1931 XYZ space, roughly representing the long-wavelength receptors in
    /// the human eye: the red receptors. Usually between 0 and 1, but can range more than that.
    pub x: f64,
    /// The Y axis of the CIE 1931 XYZ space, roughly representing the middle-wavelength receptors in
    /// the human eye. In CIE 1931, this is fudged to correspond exactly with perceived luminance.
    pub y: f64,
    /// The Z axis of the CIE 1931 XYZ space, roughly representing the short-wavelength receptors in
    /// the human eye. Usually between 0 and 1, but can range more than that.
    pub z: f64,
    /// The illuminant that is assumed to be the lighting environment for this color. Although XYZ
    /// itself describes the human response to a color and so is independent of lighting, it is useful
    /// to consider the question "how would an object in one light look different in another?" and so,
    /// to contain all the information needed to track this, the illuminant is set. Don't modify this
    /// directly in most cases: use the `color_adapt` function to do it.
    pub illuminant: Illuminant,
}

impl XYZColor {
    /// Transforms a given XYZ coordinate to the Bradford RGB space.
    fn bradford_transform(xyz: [f64; 3]) -> [f64; 3] {
        let r = 00.8951 * xyz[0] + 0.2664 * xyz[1] - 0.1614 * xyz[2];
        let g = -0.7502 * xyz[0] + 1.7135 * xyz[1] + 0.0367 * xyz[2];
        let b = 00.0389 * xyz[0] - 0.0685 * xyz[1] + 1.0296 * xyz[2];
        [r, g, b]
    }
    pub fn color_adapt(&self, other_illuminant: Illuminant) -> XYZColor {
        // no need to transform if same illuminant
        if other_illuminant == self.illuminant {
            *self
        }
        else {
            // convert to Bradford RGB space
            let rgb = XYZColor::bradford_transform([self.x, self.y, self.z]);

            // get the RGB values for the white point of the illuminant we are currently using and
            // the one we want: wr here stands for "white reference", i.e., the one we're converting
            // to
            let rgb_w = XYZColor::bradford_transform(self.illuminant.white_point());
            let rgb_wr = XYZColor::bradford_transform(other_illuminant.white_point());

            // perform the transform
            // this usually includes a parameter indicating how much you want to adapt, but it's
            // assumed that we want total adaptation: D = 1. Maybe this could change someday?

            // because each white point has already been normalized to Y = 1, we don't need ap
            // factor for it, which simplifies calculation even more than setting D = 1 and makes it
            // just a linear transform
            let r_c = rgb[0] * rgb_wr[0] / rgb_w[0];
            let g_c = rgb[1] * rgb_wr[1] / rgb_w[1];
            // there's a slight nonlinearity here that I will omit
            let b_c = rgb[2] * (rgb_wr[2] / rgb_w[2]);

            // convert back to XYZ using closer matrix inverse than before
            let x_c = 00.986993 * r_c - 0.147054 * g_c + 0.159963 * b_c;
            let y_c = 00.432305 * r_c + 0.518360 * g_c + 0.049291 * b_c;
            let z_c = -0.008529 * r_c + 0.040043 * g_c + 0.968487 * b_c;
            XYZColor{x: x_c, y: y_c, z: z_c, illuminant: other_illuminant}
        }
    }
    /// Returns `true` if the given other XYZ color's coordinates are all within 0.001 of each other,
    /// which helps account for necessary floating-point errors in conversions.
    pub fn approx_equal(&self, other: &XYZColor) -> bool {
        ((self.x - other.x).abs() <= 0.001 &&
         (self.y - other.y).abs() <= 0.001 &&
         (self.z - other.z).abs() <= 0.001)
    }
        
    /// Returns `true` if the given other XYZ color would look identically in a different color
    /// space. Uses an approximate float equality that helps resolve errors due to floating-point
    /// representation, only testing if the two floats are within 0.001 of each other.
    pub fn approx_visually_equal(&self, other: &XYZColor) -> bool {
        let other_c = other.color_adapt(self.illuminant);
        self.approx_equal(&other_c)
    }
    /// Gets the XYZColor corresponding to pure white in the given light environment.
    pub fn white_point(illuminant: Illuminant) -> XYZColor {
        let wp = illuminant.white_point();
        XYZColor{x: wp[0], y: wp[1], z: wp[2], illuminant}
    }
}

/// A trait that includes any color representation that can be converted to and from the CIE 1931 XYZ
/// color space.
pub trait Color {
    /// Converts from a color in CIE 1931 XYZ to the given color type.
    fn from_xyz(XYZColor) -> Self;
    /// Converts from the given color type to a color in CIE 1931 XYZ space. Because most color types
    /// don't include illuminant information, it is provided instead, as an enum. For most
    /// applications, D50 or D65 is a good choice.
    fn to_xyz(&self, illuminant: Illuminant) -> XYZColor;

    /// Converts the given Color to a different Color type, without consuming the curreppnt color. `T`
    /// is the color that is being converted to.  This currently converts back and forth using the
    /// D50 standard illuminant. However, this shouldn't change the actual value if the color
    /// conversion methods operate correctly, and this value should not be relied upon and can be
    /// changed without notice.
    fn convert<T: Color>(&self) -> T {
        // theoretically, the illuminant shouldn't matter as long as the color conversions are
        // correct. D50 is a common gamut for use in internal conversions, so for spaces like CIELAB
        // it will produce the least error
        T::from_xyz(self.to_xyz(Illuminant::D50))
    }
    /// "Colors" a given piece of text with terminal escape codes to allow it to be printed out in the
    /// given foreground color. Will cause problems with terminals that do not support truecolor.
    fn write_colored_str(&self, text: &str) -> String {
        let rgb: RGBColor = self.convert();
        rgb.base_write_colored_str(text)
    }
    /// Returns a string which, when printed in a truecolor-supporting terminal, will hopefully have
    /// both the foreground and background of the desired color, appearing as a complete square.
    fn write_color(&self) -> String {
        let rgb: RGBColor = self.convert();
        rgb.base_write_color()
    }
}

impl Color for XYZColor {
    fn from_xyz(xyz: XYZColor) -> XYZColor {
        xyz
    }
    #[allow(unused_variables)]
    fn to_xyz(&self, illuminant: Illuminant) -> XYZColor {
        *self
    }
}

#[derive(Debug, Copy, Clone, Eq)]
pub struct RGBColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    // TODO: add exact unclamped versions of each of these
}
    
impl RGBColor {
    /// Given a string, returns that string wrapped in codes that will color the foreground. Used for
    /// the trait implementation of write_colored_str, which should be used instead.
    fn base_write_colored_str(&self, text: &str) -> String {
        format!("{code}{text}{reset}",
                code=Fg(Rgb(self.r, self.g, self.b)),
                text=text,
                reset=Fg(Reset)
        )
    }
    fn base_write_color(&self) -> String {
        format!("{bg}{fg}{text}{reset_fg}{reset_bg}",
                bg=Bg(Rgb(self.r, self.g, self.b)),
                fg=Fg(Rgb(self.r, self.g, self.b)),
                text="■",
                reset_fg=Fg(Reset),
                reset_bg=Bg(Reset),
        )
    }
}
// TODO: get RGB from string

impl PartialEq for RGBColor {
    fn eq(&self, other: &RGBColor) -> bool {
        self.r == other.r && self.g == other.g && self.b == other.b
    }
}
        

impl From<(u8, u8, u8)> for RGBColor {
    fn from(rgb: (u8, u8, u8)) -> RGBColor {
        let (r, g, b) = rgb;
        RGBColor{r, g, b}
    }
}

impl Into<(u8, u8, u8)> for RGBColor {
    fn into(self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }
}

impl ToString for RGBColor {
    fn to_string(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

impl Color for RGBColor {
    fn from_xyz(xyz: XYZColor) -> RGBColor {
        // sRGB uses D65 as the assumed illuminant: convert the given value to that
        let xyz_d65 = xyz.color_adapt(Illuminant::D65);
        // first, get linear RGB values (i.e., without gamma correction)
        // https://en.wikipedia.org/wiki/SRGB#Specification_of_the_transformation

        // note how the diagonals are large: X, Y, Z, roughly equivalent to R, G, B
        let rgb_lin_vec = vec![3.2406 * xyz_d65.x - 1.5372 * xyz_d65.y - 0.4986 * xyz_d65.z,
                               -0.9689 * xyz_d65.x + 1.8758 * xyz_d65.y + 0.0415 * xyz_d65.z,
                               0.0557 * xyz_d65.x - 0.2040 * xyz_d65.y + 1.0570 * xyz_d65.z];
        // now we scale for gamma correction
        let gamma_correct = |x: &f64| {
            if x <= &0.0031308 {
                &12.92 * x
            }
            else {
                &1.055 * x.powf(&1.0 / &2.4) - &0.055
            }
        };
        let float_vec:Vec<f64> = rgb_lin_vec.iter().map(gamma_correct).collect();
        // now rescale between 0 and 255 and cast to integers
        // TODO: deal with clamping and exact values
        // we're going to clamp values to between 0 and 255
        let clamp = |x: &f64| {
            if *x >= 1.0 {
                1.0
            } else if *x <= 0.0 {
                0.0
            } else {
                *x
            }
        };
        let rgb:Vec<u8> = float_vec.iter().map(clamp).map(|x| (x * 255.0).round() as u8).collect();
        
        RGBColor {
            r: rgb[0],
            g: rgb[1],
            b: rgb[2]
        }
    }

    fn to_xyz(&self, illuminant: Illuminant) -> XYZColor {
        // scale from 0 to 1 instead
        // TODO: use exact values here?
        let uncorrect_gamma = |x: &f64| {
            if x <= &0.04045 {
                x / &12.92
            }
            else {
                ((x + &0.055) / &1.055).powf(2.4)
            }
        };
        let scaled_vec: Vec<f64> = vec![self.r, self.g, self.b].iter().map(|x| (*x as f64) / 255.0).collect();
        let rgb_vec: Vec<f64> = scaled_vec.iter().map(uncorrect_gamma).collect();

        // essentially the inverse of the above matrix multiplication
        let x = 0.4124 * rgb_vec[0] + 0.3576 * rgb_vec[1] + 0.1805 * rgb_vec[2];
        let y = 0.2126 * rgb_vec[0] + 0.7152 * rgb_vec[1] + 0.0722 * rgb_vec[2];
        let z = 0.0193 * rgb_vec[0] + 0.1192 * rgb_vec[1] + 0.9505 * rgb_vec[2];

        // sRGB, which this is based on, uses D65 as white, but you can convert to whatever
        // illuminant is specified
        let converted = XYZColor{x, y, z, illuminant: Illuminant::D65};
        converted.color_adapt(illuminant)        
    }
}

/// An error type that results from an invalid attempt to convert a string into an RGB color.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum RGBParseError {
    /// This indicates that function syntax was acceptable, but the numbers were out of range, such as
    /// the invalid string `"rgb(554, 23, 553)"`.
    OutOfRange,
    /// This indicates that the hex string was malformed in some way.
    InvalidHexSyntax,
    /// This indicates a syntax error in the string that was supposed to be a valid rgb( function.
    InvalidFuncSyntax,
    /// This indicated an invalid color name was supplied to the `from_color_name()` function.
    InvalidX11Name
}

impl From<ParseIntError> for RGBParseError {
    fn from(_err: ParseIntError) -> RGBParseError {
        RGBParseError::OutOfRange
    }
}

impl RGBColor {
    /// Given a string that represents a hex code, returns the RGB color that the given hex code
    /// represents. Four formats are accepted: `"#rgb"` as a shorthand for `"#rrggbb"`, `#rrggbb` by
    /// itself, and either of those formats without `#`: `"rgb"` or `"rrggbb"` are acceptable. Returns
    /// a ColorParseError if the given string does not follow one of these formats.
    pub fn from_hex_code(hex: &str) -> Result<RGBColor, RGBParseError> {
        let mut chars: Vec<char> = hex.chars().collect();
        // check if leading hex, remove if so
        if chars[0] == '#' {
            chars.remove(0);
        }
        // can only have 3 or 6 characters: error if not so
        if chars.len() != 3 && chars.len() != 6 {
            Err(RGBParseError::InvalidHexSyntax)
            // now split on invalid hex
        } else if !chars.iter().all(|&c| "0123456789ABCDEFabcdef".contains(c)) {
            Err(RGBParseError::InvalidHexSyntax)
        } else {
            // split on whether it's #rgb or #rrggbb
            if chars.len() == 6 {
                let mut rgb: Vec<u8> = Vec::new();
                for _i in 0..3 {
                    // this should never fail, logically, but if by some miracle it did it'd just
                    // return an OutOfRangeError
                    rgb.push(u8::from_str_radix(chars.drain(..2).collect::<String>().as_str(), 16).unwrap());
                }
                Ok(RGBColor{r: rgb[0], g: rgb[1], b: rgb[2]})
            }
            else { // len must be 3 from earlier
                let mut rgb: Vec<u8> = Vec::new();
                for _i in 0..3 {
                    // again, this shouldn't ever fail, but if it did it'd just return an
                    // OutOfRangeError
                    let c: Vec<char> = chars.drain(..1).collect();
                    rgb.push(u8::from_str_radix(c.iter().chain(c.iter()).collect::<String>().as_str(), 16).unwrap());
                }
                Ok(RGBColor{r: rgb[0], g: rgb[1], b: rgb[2]})
            }
        }
    }
    /// Gets the RGB color corresponding to an X11 color name. Case is ignored.
    pub fn from_color_name(name: &str) -> Result<RGBColor, RGBParseError> {
        // this is the full list of X11 color names
        // I used a Python script to process it from this site:
        // https://github.com/bahamas10/css-color-names/blob/master/css-color-names.json let
        // I added the special "transparent" referring to #00000000
        let color_names:Vec<&str> = [
            "aliceblue", "antiquewhite", "aqua", "aquamarine", "azure", "beige",
            "bisque", "black", "blanchedalmond", "blue", "blueviolet", "brown", "burlywood", "cadetblue",
            "chartreuse", "chocolate", "coral", "cornflowerblue", "cornsilk", "crimson", "cyan", "darkblue",
            "darkcyan", "darkgoldenrod", "darkgray", "darkgreen", "darkgrey", "darkkhaki", "darkmagenta",
            "darkolivegreen", "darkorange", "darkorchid", "darkred", "darksalmon", "darkseagreen",
            "darkslateblue", "darkslategray", "darkslategrey", "darkturquoise", "darkviolet", "deeppink",
            "deepskyblue", "dimgray", "dimgrey", "dodgerblue", "firebrick", "floralwhite", "forestgreen",
            "fuchsia", "gainsboro", "ghostwhite", "gold", "goldenrod", "gray", "green", "greenyellow",
            "grey", "honeydew", "hotpink", "indianred", "indigo", "ivory", "khaki", "lavender",
            "lavenderblush", "lawngreen", "lemonchiffon", "lightblue", "lightcoral", "lightcyan",
            "lightgoldenrodyellow", "lightgray", "lightgreen", "lightgrey", "lightpink", "lightsalmon",
            "lightseagreen", "lightskyblue", "lightslategray", "lightslategrey", "lightsteelblue",
            "lightyellow", "lime", "limegreen", "linen", "magenta", "maroon", "mediumaquamarine",
            "mediumblue", "mediumorchid", "mediumpurple", "mediumseagreen", "mediumslateblue",
            "mediumspringgreen", "mediumturquoise", "mediumvioletred", "midnightblue", "mintcream",
            "mistyrose", "moccasin", "navajowhite", "navy", "oldlace", "olive", "olivedrab", "orange",
            "orangered", "orchid", "palegoldenrod", "palegreen", "paleturquoise", "palevioletred",
            "papayawhip", "peachpuff", "peru", "pink", "plum", "powderblue", "purple", "rebeccapurple",
            "red", "rosybrown", "royalblue", "saddlebrown", "salmon", "sandybrown", "seagreen", "seashell",
            "sienna", "silver", "skyblue", "slateblue", "slategray", "slategrey", "snow", "springgreen",
            "steelblue", "tan", "teal", "thistle", "tomato", "turquoise", "violet", "wheat", "white",
            "whitesmoke", "yellow", "yellowgreen"
        ].to_vec();
        let color_codes:Vec<&str> = [
            "#f0f8ff", "#faebd7", "#00ffff", "#7fffd4", "#f0ffff", "#f5f5dc", "#ffe4c4", "#000000",
            "#ffebcd", "#0000ff", "#8a2be2", "#a52a2a", "#deb887", "#5f9ea0", "#7fff00", "#d2691e",
            "#ff7f50", "#6495ed", "#fff8dc", "#dc143c", "#00ffff", "#00008b", "#008b8b", "#b8860b",
            "#a9a9a9", "#006400", "#a9a9a9", "#bdb76b", "#8b008b", "#556b2f", "#ff8c00", "#9932cc",
            "#8b0000", "#e9967a", "#8fbc8f", "#483d8b", "#2f4f4f", "#2f4f4f", "#00ced1", "#9400d3",
            "#ff1493", "#00bfff", "#696969", "#696969", "#1e90ff", "#b22222", "#fffaf0", "#228b22",
            "#ff00ff", "#dcdcdc", "#f8f8ff", "#ffd700", "#daa520", "#808080", "#008000", "#adff2f",
            "#808080", "#f0fff0", "#ff69b4", "#cd5c5c", "#4b0082", "#fffff0", "#f0e68c", "#e6e6fa",
            "#fff0f5", "#7cfc00", "#fffacd", "#add8e6", "#f08080", "#e0ffff", "#fafad2", "#d3d3d3",
            "#90ee90", "#d3d3d3", "#ffb6c1", "#ffa07a", "#20b2aa", "#87cefa", "#778899", "#778899",
            "#b0c4de", "#ffffe0", "#00ff00", "#32cd32", "#faf0e6", "#ff00ff", "#800000", "#66cdaa",
            "#0000cd", "#ba55d3", "#9370db", "#3cb371", "#7b68ee", "#00fa9a", "#48d1cc", "#c71585",
            "#191970", "#f5fffa", "#ffe4e1", "#ffe4b5", "#ffdead", "#000080", "#fdf5e6", "#808000",
            "#6b8e23", "#ffa500", "#ff4500", "#da70d6", "#eee8aa", "#98fb98", "#afeeee", "#db7093",
            "#ffefd5", "#ffdab9", "#cd853f", "#ffc0cb", "#dda0dd", "#b0e0e6", "#800080", "#663399",
            "#ff0000", "#bc8f8f", "#4169e1", "#8b4513", "#fa8072", "#f4a460", "#2e8b57", "#fff5ee",
            "#a0522d", "#c0c0c0", "#87ceeb", "#6a5acd", "#708090", "#708090", "#fffafa", "#00ff7f",
            "#4682b4", "#d2b48c", "#008080", "#d8bfd8", "#ff6347", "#40e0d0", "#ee82ee", "#f5deb3",
            "#ffffff", "#f5f5f5", "#ffff00", "#9acd32"
        ].to_vec();
        let mut names_to_codes = HashMap::new();

        for (i, color_name) in color_names.iter().enumerate() {
            names_to_codes.insert(color_name, color_codes[i]);
        }

        // now just return the converted value or raise one if not in hashmap
        match names_to_codes.get(&name.to_lowercase().as_str()) {
            None => Err(RGBParseError::InvalidX11Name),
            Some(x) => Self::from_hex_code(x)
        }
    }
}

/// Describes a Color that can be mixed with other colors in its own 3D space. Mixing, in this
/// context, is taking the midpoint of two color projections in some space, or something consistent
/// with that idea: if colors A and B mix to A, that should mean B is the same as A, for
/// example. Although this is not currently the case, note that this implies that the gamut of this
/// Color is convex: any two Colors of the same type may be mixed to form a third valid one.

/// Note that there is one very crucial thing to remember about mixing: it differs depending on the
/// color space being used. For example, if there are two colors A and B, A.mix(B) may produce very
/// different results than A_conv.mix(B_conv) if A_conv and B_conv are the results of A.convert() and
/// B.convert(). For this reason, A.mix(B) is only allowed if A and B share a type: otherwise,
/// A.mix(B) could be different than B.mix(A), which is error-prone and unintuitive.

/// There is a default implementation for Colors that can interconvert to Coord. This helps ensure
/// that the most basic case functions appropriately. For any other type of Color, special logic is
/// needed because of range and rounding issues, so it's on the type itself to implement it.

/// Especially note that color mixing as one thinks of with paints or other subtractive mixtures will
/// almost definitely not agree with the output of Scarlet, because computer monitors use additive
/// mixing while pigments use subtractive mixing. Yellow mixed with blue in most RGB or other systems
/// is gray, not green

pub trait Mix : Color {
    /// Given two Colors, returns a Color representing their midpoint: usually, this means their
    /// midpoint in some projection into three-dimensional space.
    fn mix(self, other: Self) -> Self;
}

impl<T: Color + From<Coord> + Into<Coord>> Mix for T {
    /// Given two colors that represent the points (a1, b1, c1) and (a2, b2, c2) in some common
    /// projection, returns the color (a1 + a2, b1 + b2, c1 + c2) / 2.
    fn mix(self, other: T) -> T {
        // convert to 3D space, add, divide by 2, come back
        let c1: Coord = self.into();
        let c2: Coord = other.into();
        T::from((c1 + c2) / 2)
    }        
}

// `XYZColor` notably doesn't implement conversion to and from `Coord` because illuminant information
// can't be preserved: this means that mixing colors with different illuminants would produce
// incorrect results. The following custom implementation of the Mix trait fixes this by converting
// colors to the same gamut first.
impl Mix for XYZColor {
    /// Uses the current XYZ illuminant as the base, and uses the chromatic adapation transform that
    /// the `XYZColor` struct defines (as `color_adapt`).
    fn mix(self, other: XYZColor) -> XYZColor {
        // convert to same illuminant
        let other_c = other.color_adapt(self.illuminant);
        // now just take the midpoint in 3D space
        let c1: Coord = Coord{x: self.x, y: self.y, z: self.z};
        let c2: Coord = Coord{x: other_c.x, y: other_c.y, z: other_c.z};
        let mixed_coord = (c1 + c2) / 2.0;
        XYZColor{
            x: mixed_coord.x,
            y: mixed_coord.y,
            z: mixed_coord.z,
            illuminant: self.illuminant
        }
    }
}

impl Mix for RGBColor {
    fn mix(self, other: RGBColor) -> RGBColor {
        let (r1, g1, b1) = self.into();
        let (r2, g2, b2) = other.into();
        let (r, g, b) = (((r1 as u16 + r2 as u16) / 2) as u8,
                         ((g1 as u16 + g2 as u16) / 2) as u8,
                         ((b1 as u16 + b2 as u16) / 2) as u8);
        RGBColor{r, g, b}
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn can_display_colors() {
        let b = 128;
        for i in 0..8 {
            let mut line = String::from("");
            let r = i * 16;
            for j in 0..8 {
                let g = j * 16;
                line.push_str(RGBColor{r, g, b}.write_colored_str("■").as_str());                
            }
            println!("{}", line);        }
    }
    
    #[test]
    fn xyz_to_rgb() {
        let xyz = XYZColor{x: 0.41874, y: 0.21967, z: 0.05649, illuminant: Illuminant::D65};
        let rgb: RGBColor = xyz.convert();
        assert_eq!(rgb.r, 254);
        assert_eq!(rgb.g, 23);
        assert_eq!(rgb.b, 55);
    }

    #[test]
    fn rgb_to_xyz() {
        let rgb = RGBColor{r: 45, g: 28, b: 156};
        let xyz: XYZColor = rgb.to_xyz(Illuminant::D65);
        // these won't match exactly cuz floats, so I just check within a margin
        assert!((xyz.x - 0.0750).abs() <= 0.01);
        assert!((xyz.y - 0.0379).abs() <= 0.01);
        assert!((xyz.z-  0.3178).abs() <= 0.01);
    }
    // for now, not gonna use since the fun color adaptation demo already runs this
    #[allow(dead_code)]
    fn test_xyz_color_display() {
        println!();
        let y = 0.5;
        for i in 0..21 {
            let mut line = String::from("");
            for j in 0..21 {
                let x = i as f64 * 0.8 / 20.0;
                let z = j as f64 * 0.8 / 20.0;
                line.push_str(XYZColor{x, y, z, illuminant: Illuminant::D65}.write_colored_str("■").as_str());
            }

            println!("{}", line);
        }
    }
    #[test]
    fn test_rgb_to_string() {
        let c1 = RGBColor{r: 0, g: 0, b: 0};
        let c2 = RGBColor{r: 244, g: 182, b: 33};
        let c3 = RGBColor{r: 0, g: 255, b: 0};
        assert_eq!(c1.to_string(), "#000000");
        assert_eq!(c2.to_string(), "#F4B621");
        assert_eq!(c3.to_string(), "#00FF00");
    }
    #[test]
    fn test_mix_rgb() {
        let c1 = RGBColor::from((0, 0, 255));
        let c2 = RGBColor::from((255, 0, 1));
        let c3 = RGBColor::from((127, 7, 19));
        assert_eq!(c1.mix(c2).to_string(), "#7F0080");
        assert_eq!(c1.mix(c3).to_string(), "#3F0389");
        assert_eq!(c2.mix(c3).to_string(), "#BF030A");
    }
    #[test]
    fn test_mix_xyz() {
        // note how I'm using fractions with powers of 2 in the denominator to prevent floating-point issues
        let c1 = XYZColor{x: 0.5, y: 0.25, z: 0.75, illuminant: Illuminant::D65};
        let c2 = XYZColor{x: 0.625, y: 0.375, z: 0.5, illuminant: Illuminant::D65};
        let c3 = XYZColor{x: 0.75, y: 0.5, z: 0.25, illuminant: Illuminant::D65};
        assert_eq!(c1.mix(c3), c2);
        assert_eq!(c3.mix(c1), c2);
    }
    #[test]
    fn test_xyz_color_adaptation() {
        // I can literally not find a single API or something that does this so I can check the
        // values, so I'll just hope that it's good enough to check that converting between several
        // illuminants and back again gets something good
        let c1 = XYZColor{x: 0.5, y: 0.75, z: 0.6, illuminant: Illuminant::D65};
        let c2 = c1.color_adapt(Illuminant::D50).color_adapt(Illuminant::D55);
        let c3 = c1.color_adapt(Illuminant::D75).color_adapt(Illuminant::D55);
        println!("{} {} {}", c1.x, c1.y, c1.z);
        println!("{} {} {}", c2.x, c2.y, c2.z);
        println!("{} {} {}", c3.x, c3.y, c3.z);
        assert!((c3.x - c2.x).abs() <= 0.01);
        assert!((c3.y - c2.y).abs() <= 0.01);
        assert!((c3.z - c2.z).abs() <= 0.01);
    }
    #[test]
    fn test_chromatic_adapation_to_same_light() {
        let xyz = XYZColor{x: 0.4, y: 0.6, z: 0.2, illuminant: Illuminant::D65};
        let xyz2 = xyz.color_adapt(Illuminant::D65);
        assert_eq!(xyz, xyz2);
    }
    #[test]
    fn fun_color_adaptation_demo() {
        println!();
        let w: usize = 120;
        let h: usize = 60;
        let d50_wp = Illuminant::D50.white_point();
        let d75_wp = Illuminant::D75.white_point();
        let d50 = XYZColor{x: d50_wp[0], y: d50_wp[1], z: d50_wp[2],
                           illuminant:Illuminant::D65};
        let d75 = XYZColor{x: d75_wp[0], y: d75_wp[1], z: d75_wp[2],
                           illuminant:Illuminant::D65};
        for _ in 0..h+1 {
            println!("{}{}", d50.write_color().repeat(w / 2), d75.write_color().repeat(w / 2));
        }
        
        println!();
        println!();
        let y = 0.5;
        println!();
        for i in 0..(h+1) {
            let mut line = String::from("");
            let x = i as f64 * 0.9 / h as f64;
            for j in 0..(w / 2) {
                let z = j as f64 * 0.9 / w as f64;
                line.push_str(XYZColor{x, y, z, illuminant: Illuminant::D50}.write_color().as_str());
            }
            for j in (w / 2)..(w+1) {
                let z = j as f64 * 0.9 / w as f64;
                line.push_str(XYZColor{x, y, z, illuminant: Illuminant::D75}.write_color().as_str());
            }
            println!("{}", line);
        }
        println!();
        println!();
        for i in 0..(h+1) {
            let mut line = String::from("");
            let x = i as f64 * 0.9 / h as f64;
            for j in 0..w {
                let z = j as f64 * 0.9 / w as f64;
                line.push_str(XYZColor{x, y, z, illuminant: Illuminant::D65}.write_color().as_str());
            }
            println!("{}", line);
        }
    }
    #[test]
    fn test_rgb_from_hex() {
        // test rgb format
        let rgb = RGBColor::from_hex_code("#172844").unwrap();
        assert_eq!(rgb.r, 23);
        assert_eq!(rgb.g, 40);
        assert_eq!(rgb.b, 68);
        // test with letters and no hex
        let rgb = RGBColor::from_hex_code("a1F1dB").unwrap();
        assert_eq!(rgb.r, 161);
        assert_eq!(rgb.g, 241);
        assert_eq!(rgb.b, 219);
        // test for error if 7 chars
        let rgb = RGBColor::from_hex_code("#1244444");
        assert!(match rgb {
            Err(x) if x == RGBParseError::InvalidHexSyntax => true,
            _ => false
        });
        // test for error if invalid hex chars
        let rgb = RGBColor::from_hex_code("#ffggbb");
        assert!(match rgb {
            Err(x) if x == RGBParseError::InvalidHexSyntax => true,
            _ => false
        });               
    }
    #[test]
    fn test_rgb_from_name() {
        let rgb = RGBColor::from_color_name("yeLlowgreEn").unwrap();
        assert_eq!(rgb.r, 154);
        assert_eq!(rgb.g, 205);
        assert_eq!(rgb.b, 50);
        // test error
        let rgb = RGBColor::from_color_name("thisisnotavalidnamelol");
        assert!(match rgb {
            Err(x) if x == RGBParseError::InvalidX11Name => true,
            _ => false
        });
    }
    #[test]
    fn test_to_string() {
        for hex in ["#000000", "#ABCDEF", "#1A2B3C", "#D00A12", "#40AA50"].iter() {
            assert_eq!(*hex, RGBColor::from_hex_code(hex).unwrap().to_string());
        }
    }
}
