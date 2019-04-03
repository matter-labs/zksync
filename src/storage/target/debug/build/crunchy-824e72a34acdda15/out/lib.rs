
/// Unroll the given for loop
///
/// Example:
///
/// ```ignore
/// unroll! {
///   for i in 0..5 {
///     println!("Iteration {}", i);
///   }
/// }
/// ```
///
/// will expand into:
///
/// ```ignore
/// { println!("Iteration {}", 0); }
/// { println!("Iteration {}", 1); }
/// { println!("Iteration {}", 2); }
/// { println!("Iteration {}", 3); }
/// { println!("Iteration {}", 4); }
/// ```
#[macro_export]
macro_rules! unroll {
    (for $v:ident in 0..0 $c:block) => {};

    (for $v:ident in 0..$b:tt {$($c:tt)*}) => {
        #[allow(non_upper_case_globals)]
        { unroll!(@$v, 0, $b, {$($c)*}); }
    };

    (@$v:ident, $a:expr, 0, $c:block) => {
        { const $v: usize = $a; $c }
    };

    (@$v:ident, $a:expr, 1, $c:block) => {
        { const $v: usize = $a; $c }
    };

    (@$v:ident, $a:expr, 2, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
    };

    (@$v:ident, $a:expr, 3, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
    };

    (@$v:ident, $a:expr, 4, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
    };

    (@$v:ident, $a:expr, 5, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
    };

    (@$v:ident, $a:expr, 6, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
    };

    (@$v:ident, $a:expr, 7, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
    };

    (@$v:ident, $a:expr, 8, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
    };

    (@$v:ident, $a:expr, 9, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
        { const $v: usize = $a + 8; $c }
    };

    (@$v:ident, $a:expr, 10, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
        { const $v: usize = $a + 8; $c }
        { const $v: usize = $a + 9; $c }
    };

    (@$v:ident, $a:expr, 11, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
        { const $v: usize = $a + 8; $c }
        { const $v: usize = $a + 9; $c }
        { const $v: usize = $a + 10; $c }
    };

    (@$v:ident, $a:expr, 12, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
        { const $v: usize = $a + 8; $c }
        { const $v: usize = $a + 9; $c }
        { const $v: usize = $a + 10; $c }
        { const $v: usize = $a + 11; $c }
    };

    (@$v:ident, $a:expr, 13, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
        { const $v: usize = $a + 8; $c }
        { const $v: usize = $a + 9; $c }
        { const $v: usize = $a + 10; $c }
        { const $v: usize = $a + 11; $c }
        { const $v: usize = $a + 12; $c }
    };

    (@$v:ident, $a:expr, 14, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
        { const $v: usize = $a + 8; $c }
        { const $v: usize = $a + 9; $c }
        { const $v: usize = $a + 10; $c }
        { const $v: usize = $a + 11; $c }
        { const $v: usize = $a + 12; $c }
        { const $v: usize = $a + 13; $c }
    };

    (@$v:ident, $a:expr, 15, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
        { const $v: usize = $a + 8; $c }
        { const $v: usize = $a + 9; $c }
        { const $v: usize = $a + 10; $c }
        { const $v: usize = $a + 11; $c }
        { const $v: usize = $a + 12; $c }
        { const $v: usize = $a + 13; $c }
        { const $v: usize = $a + 14; $c }
    };

    (@$v:ident, $a:expr, 16, $c:block) => {
        { const $v: usize = $a; $c }
        { const $v: usize = $a + 1; $c }
        { const $v: usize = $a + 2; $c }
        { const $v: usize = $a + 3; $c }
        { const $v: usize = $a + 4; $c }
        { const $v: usize = $a + 5; $c }
        { const $v: usize = $a + 6; $c }
        { const $v: usize = $a + 7; $c }
        { const $v: usize = $a + 8; $c }
        { const $v: usize = $a + 9; $c }
        { const $v: usize = $a + 10; $c }
        { const $v: usize = $a + 11; $c }
        { const $v: usize = $a + 12; $c }
        { const $v: usize = $a + 13; $c }
        { const $v: usize = $a + 14; $c }
        { const $v: usize = $a + 15; $c }
    };

    (@$v:ident, $a:expr, 17, $c:block) => {
        unroll!(@$v, $a, 16, $c);
        { const $v: usize = $a + 16; $c }
    };

    (@$v:ident, $a:expr, 18, $c:block) => {
        unroll!(@$v, $a, 9, $c);
        unroll!(@$v, $a + 9, 9, $c);
    };

    (@$v:ident, $a:expr, 19, $c:block) => {
        unroll!(@$v, $a, 18, $c);
        { const $v: usize = $a + 18; $c }
    };

    (@$v:ident, $a:expr, 20, $c:block) => {
        unroll!(@$v, $a, 10, $c);
        unroll!(@$v, $a + 10, 10, $c);
    };

    (@$v:ident, $a:expr, 21, $c:block) => {
        unroll!(@$v, $a, 20, $c);
        { const $v: usize = $a + 20; $c }
    };

    (@$v:ident, $a:expr, 22, $c:block) => {
        unroll!(@$v, $a, 11, $c);
        unroll!(@$v, $a + 11, 11, $c);
    };

    (@$v:ident, $a:expr, 23, $c:block) => {
        unroll!(@$v, $a, 22, $c);
        { const $v: usize = $a + 22; $c }
    };

    (@$v:ident, $a:expr, 24, $c:block) => {
        unroll!(@$v, $a, 12, $c);
        unroll!(@$v, $a + 12, 12, $c);
    };

    (@$v:ident, $a:expr, 25, $c:block) => {
        unroll!(@$v, $a, 24, $c);
        { const $v: usize = $a + 24; $c }
    };

    (@$v:ident, $a:expr, 26, $c:block) => {
        unroll!(@$v, $a, 13, $c);
        unroll!(@$v, $a + 13, 13, $c);
    };

    (@$v:ident, $a:expr, 27, $c:block) => {
        unroll!(@$v, $a, 26, $c);
        { const $v: usize = $a + 26; $c }
    };

    (@$v:ident, $a:expr, 28, $c:block) => {
        unroll!(@$v, $a, 14, $c);
        unroll!(@$v, $a + 14, 14, $c);
    };

    (@$v:ident, $a:expr, 29, $c:block) => {
        unroll!(@$v, $a, 28, $c);
        { const $v: usize = $a + 28; $c }
    };

    (@$v:ident, $a:expr, 30, $c:block) => {
        unroll!(@$v, $a, 15, $c);
        unroll!(@$v, $a + 15, 15, $c);
    };

    (@$v:ident, $a:expr, 31, $c:block) => {
        unroll!(@$v, $a, 30, $c);
        { const $v: usize = $a + 30; $c }
    };

    (@$v:ident, $a:expr, 32, $c:block) => {
        unroll!(@$v, $a, 16, $c);
        unroll!(@$v, $a + 16, 16, $c);
    };

    (@$v:ident, $a:expr, 33, $c:block) => {
        unroll!(@$v, $a, 32, $c);
        { const $v: usize = $a + 32; $c }
    };

    (@$v:ident, $a:expr, 34, $c:block) => {
        unroll!(@$v, $a, 17, $c);
        unroll!(@$v, $a + 17, 17, $c);
    };

    (@$v:ident, $a:expr, 35, $c:block) => {
        unroll!(@$v, $a, 34, $c);
        { const $v: usize = $a + 34; $c }
    };

    (@$v:ident, $a:expr, 36, $c:block) => {
        unroll!(@$v, $a, 18, $c);
        unroll!(@$v, $a + 18, 18, $c);
    };

    (@$v:ident, $a:expr, 37, $c:block) => {
        unroll!(@$v, $a, 36, $c);
        { const $v: usize = $a + 36; $c }
    };

    (@$v:ident, $a:expr, 38, $c:block) => {
        unroll!(@$v, $a, 19, $c);
        unroll!(@$v, $a + 19, 19, $c);
    };

    (@$v:ident, $a:expr, 39, $c:block) => {
        unroll!(@$v, $a, 38, $c);
        { const $v: usize = $a + 38; $c }
    };

    (@$v:ident, $a:expr, 40, $c:block) => {
        unroll!(@$v, $a, 20, $c);
        unroll!(@$v, $a + 20, 20, $c);
    };

    (@$v:ident, $a:expr, 41, $c:block) => {
        unroll!(@$v, $a, 40, $c);
        { const $v: usize = $a + 40; $c }
    };

    (@$v:ident, $a:expr, 42, $c:block) => {
        unroll!(@$v, $a, 21, $c);
        unroll!(@$v, $a + 21, 21, $c);
    };

    (@$v:ident, $a:expr, 43, $c:block) => {
        unroll!(@$v, $a, 42, $c);
        { const $v: usize = $a + 42; $c }
    };

    (@$v:ident, $a:expr, 44, $c:block) => {
        unroll!(@$v, $a, 22, $c);
        unroll!(@$v, $a + 22, 22, $c);
    };

    (@$v:ident, $a:expr, 45, $c:block) => {
        unroll!(@$v, $a, 44, $c);
        { const $v: usize = $a + 44; $c }
    };

    (@$v:ident, $a:expr, 46, $c:block) => {
        unroll!(@$v, $a, 23, $c);
        unroll!(@$v, $a + 23, 23, $c);
    };

    (@$v:ident, $a:expr, 47, $c:block) => {
        unroll!(@$v, $a, 46, $c);
        { const $v: usize = $a + 46; $c }
    };

    (@$v:ident, $a:expr, 48, $c:block) => {
        unroll!(@$v, $a, 24, $c);
        unroll!(@$v, $a + 24, 24, $c);
    };

    (@$v:ident, $a:expr, 49, $c:block) => {
        unroll!(@$v, $a, 48, $c);
        { const $v: usize = $a + 48; $c }
    };

    (@$v:ident, $a:expr, 50, $c:block) => {
        unroll!(@$v, $a, 25, $c);
        unroll!(@$v, $a + 25, 25, $c);
    };

    (@$v:ident, $a:expr, 51, $c:block) => {
        unroll!(@$v, $a, 50, $c);
        { const $v: usize = $a + 50; $c }
    };

    (@$v:ident, $a:expr, 52, $c:block) => {
        unroll!(@$v, $a, 26, $c);
        unroll!(@$v, $a + 26, 26, $c);
    };

    (@$v:ident, $a:expr, 53, $c:block) => {
        unroll!(@$v, $a, 52, $c);
        { const $v: usize = $a + 52; $c }
    };

    (@$v:ident, $a:expr, 54, $c:block) => {
        unroll!(@$v, $a, 27, $c);
        unroll!(@$v, $a + 27, 27, $c);
    };

    (@$v:ident, $a:expr, 55, $c:block) => {
        unroll!(@$v, $a, 54, $c);
        { const $v: usize = $a + 54; $c }
    };

    (@$v:ident, $a:expr, 56, $c:block) => {
        unroll!(@$v, $a, 28, $c);
        unroll!(@$v, $a + 28, 28, $c);
    };

    (@$v:ident, $a:expr, 57, $c:block) => {
        unroll!(@$v, $a, 56, $c);
        { const $v: usize = $a + 56; $c }
    };

    (@$v:ident, $a:expr, 58, $c:block) => {
        unroll!(@$v, $a, 29, $c);
        unroll!(@$v, $a + 29, 29, $c);
    };

    (@$v:ident, $a:expr, 59, $c:block) => {
        unroll!(@$v, $a, 58, $c);
        { const $v: usize = $a + 58; $c }
    };

    (@$v:ident, $a:expr, 60, $c:block) => {
        unroll!(@$v, $a, 30, $c);
        unroll!(@$v, $a + 30, 30, $c);
    };

    (@$v:ident, $a:expr, 61, $c:block) => {
        unroll!(@$v, $a, 60, $c);
        { const $v: usize = $a + 60; $c }
    };

    (@$v:ident, $a:expr, 62, $c:block) => {
        unroll!(@$v, $a, 31, $c);
        unroll!(@$v, $a + 31, 31, $c);
    };

    (@$v:ident, $a:expr, 63, $c:block) => {
        unroll!(@$v, $a, 62, $c);
        { const $v: usize = $a + 62; $c }
    };

    (@$v:ident, $a:expr, 64, $c:block) => {
        unroll!(@$v, $a, 32, $c);
        unroll!(@$v, $a + 32, 32, $c);
    };

    (@$v:ident, $a:expr, 65, $c:block) => {
        unroll!(@$v, $a, 64, $c);
        { const $v: usize = $a + 64; $c }
    };

    (@$v:ident, $a:expr, 66, $c:block) => {
        unroll!(@$v, $a, 33, $c);
        unroll!(@$v, $a + 33, 33, $c);
    };

    (@$v:ident, $a:expr, 67, $c:block) => {
        unroll!(@$v, $a, 66, $c);
        { const $v: usize = $a + 66; $c }
    };

    (@$v:ident, $a:expr, 68, $c:block) => {
        unroll!(@$v, $a, 34, $c);
        unroll!(@$v, $a + 34, 34, $c);
    };

    (@$v:ident, $a:expr, 69, $c:block) => {
        unroll!(@$v, $a, 68, $c);
        { const $v: usize = $a + 68; $c }
    };

    (@$v:ident, $a:expr, 70, $c:block) => {
        unroll!(@$v, $a, 35, $c);
        unroll!(@$v, $a + 35, 35, $c);
    };

    (@$v:ident, $a:expr, 71, $c:block) => {
        unroll!(@$v, $a, 70, $c);
        { const $v: usize = $a + 70; $c }
    };

    (@$v:ident, $a:expr, 72, $c:block) => {
        unroll!(@$v, $a, 36, $c);
        unroll!(@$v, $a + 36, 36, $c);
    };

    (@$v:ident, $a:expr, 73, $c:block) => {
        unroll!(@$v, $a, 72, $c);
        { const $v: usize = $a + 72; $c }
    };

    (@$v:ident, $a:expr, 74, $c:block) => {
        unroll!(@$v, $a, 37, $c);
        unroll!(@$v, $a + 37, 37, $c);
    };

    (@$v:ident, $a:expr, 75, $c:block) => {
        unroll!(@$v, $a, 74, $c);
        { const $v: usize = $a + 74; $c }
    };

    (@$v:ident, $a:expr, 76, $c:block) => {
        unroll!(@$v, $a, 38, $c);
        unroll!(@$v, $a + 38, 38, $c);
    };

    (@$v:ident, $a:expr, 77, $c:block) => {
        unroll!(@$v, $a, 76, $c);
        { const $v: usize = $a + 76; $c }
    };

    (@$v:ident, $a:expr, 78, $c:block) => {
        unroll!(@$v, $a, 39, $c);
        unroll!(@$v, $a + 39, 39, $c);
    };

    (@$v:ident, $a:expr, 79, $c:block) => {
        unroll!(@$v, $a, 78, $c);
        { const $v: usize = $a + 78; $c }
    };

    (@$v:ident, $a:expr, 80, $c:block) => {
        unroll!(@$v, $a, 40, $c);
        unroll!(@$v, $a + 40, 40, $c);
    };

    (@$v:ident, $a:expr, 81, $c:block) => {
        unroll!(@$v, $a, 80, $c);
        { const $v: usize = $a + 80; $c }
    };

    (@$v:ident, $a:expr, 82, $c:block) => {
        unroll!(@$v, $a, 41, $c);
        unroll!(@$v, $a + 41, 41, $c);
    };

    (@$v:ident, $a:expr, 83, $c:block) => {
        unroll!(@$v, $a, 82, $c);
        { const $v: usize = $a + 82; $c }
    };

    (@$v:ident, $a:expr, 84, $c:block) => {
        unroll!(@$v, $a, 42, $c);
        unroll!(@$v, $a + 42, 42, $c);
    };

    (@$v:ident, $a:expr, 85, $c:block) => {
        unroll!(@$v, $a, 84, $c);
        { const $v: usize = $a + 84; $c }
    };

    (@$v:ident, $a:expr, 86, $c:block) => {
        unroll!(@$v, $a, 43, $c);
        unroll!(@$v, $a + 43, 43, $c);
    };

    (@$v:ident, $a:expr, 87, $c:block) => {
        unroll!(@$v, $a, 86, $c);
        { const $v: usize = $a + 86; $c }
    };

    (@$v:ident, $a:expr, 88, $c:block) => {
        unroll!(@$v, $a, 44, $c);
        unroll!(@$v, $a + 44, 44, $c);
    };

    (@$v:ident, $a:expr, 89, $c:block) => {
        unroll!(@$v, $a, 88, $c);
        { const $v: usize = $a + 88; $c }
    };

    (@$v:ident, $a:expr, 90, $c:block) => {
        unroll!(@$v, $a, 45, $c);
        unroll!(@$v, $a + 45, 45, $c);
    };

    (@$v:ident, $a:expr, 91, $c:block) => {
        unroll!(@$v, $a, 90, $c);
        { const $v: usize = $a + 90; $c }
    };

    (@$v:ident, $a:expr, 92, $c:block) => {
        unroll!(@$v, $a, 46, $c);
        unroll!(@$v, $a + 46, 46, $c);
    };

    (@$v:ident, $a:expr, 93, $c:block) => {
        unroll!(@$v, $a, 92, $c);
        { const $v: usize = $a + 92; $c }
    };

    (@$v:ident, $a:expr, 94, $c:block) => {
        unroll!(@$v, $a, 47, $c);
        unroll!(@$v, $a + 47, 47, $c);
    };

    (@$v:ident, $a:expr, 95, $c:block) => {
        unroll!(@$v, $a, 94, $c);
        { const $v: usize = $a + 94; $c }
    };

    (@$v:ident, $a:expr, 96, $c:block) => {
        unroll!(@$v, $a, 48, $c);
        unroll!(@$v, $a + 48, 48, $c);
    };

    (@$v:ident, $a:expr, 97, $c:block) => {
        unroll!(@$v, $a, 96, $c);
        { const $v: usize = $a + 96; $c }
    };

    (@$v:ident, $a:expr, 98, $c:block) => {
        unroll!(@$v, $a, 49, $c);
        unroll!(@$v, $a + 49, 49, $c);
    };

    (@$v:ident, $a:expr, 99, $c:block) => {
        unroll!(@$v, $a, 98, $c);
        { const $v: usize = $a + 98; $c }
    };

    (@$v:ident, $a:expr, 100, $c:block) => {
        unroll!(@$v, $a, 50, $c);
        unroll!(@$v, $a + 50, 50, $c);
    };

    (@$v:ident, $a:expr, 101, $c:block) => {
        unroll!(@$v, $a, 100, $c);
        { const $v: usize = $a + 100; $c }
    };

    (@$v:ident, $a:expr, 102, $c:block) => {
        unroll!(@$v, $a, 51, $c);
        unroll!(@$v, $a + 51, 51, $c);
    };

    (@$v:ident, $a:expr, 103, $c:block) => {
        unroll!(@$v, $a, 102, $c);
        { const $v: usize = $a + 102; $c }
    };

    (@$v:ident, $a:expr, 104, $c:block) => {
        unroll!(@$v, $a, 52, $c);
        unroll!(@$v, $a + 52, 52, $c);
    };

    (@$v:ident, $a:expr, 105, $c:block) => {
        unroll!(@$v, $a, 104, $c);
        { const $v: usize = $a + 104; $c }
    };

    (@$v:ident, $a:expr, 106, $c:block) => {
        unroll!(@$v, $a, 53, $c);
        unroll!(@$v, $a + 53, 53, $c);
    };

    (@$v:ident, $a:expr, 107, $c:block) => {
        unroll!(@$v, $a, 106, $c);
        { const $v: usize = $a + 106; $c }
    };

    (@$v:ident, $a:expr, 108, $c:block) => {
        unroll!(@$v, $a, 54, $c);
        unroll!(@$v, $a + 54, 54, $c);
    };

    (@$v:ident, $a:expr, 109, $c:block) => {
        unroll!(@$v, $a, 108, $c);
        { const $v: usize = $a + 108; $c }
    };

    (@$v:ident, $a:expr, 110, $c:block) => {
        unroll!(@$v, $a, 55, $c);
        unroll!(@$v, $a + 55, 55, $c);
    };

    (@$v:ident, $a:expr, 111, $c:block) => {
        unroll!(@$v, $a, 110, $c);
        { const $v: usize = $a + 110; $c }
    };

    (@$v:ident, $a:expr, 112, $c:block) => {
        unroll!(@$v, $a, 56, $c);
        unroll!(@$v, $a + 56, 56, $c);
    };

    (@$v:ident, $a:expr, 113, $c:block) => {
        unroll!(@$v, $a, 112, $c);
        { const $v: usize = $a + 112; $c }
    };

    (@$v:ident, $a:expr, 114, $c:block) => {
        unroll!(@$v, $a, 57, $c);
        unroll!(@$v, $a + 57, 57, $c);
    };

    (@$v:ident, $a:expr, 115, $c:block) => {
        unroll!(@$v, $a, 114, $c);
        { const $v: usize = $a + 114; $c }
    };

    (@$v:ident, $a:expr, 116, $c:block) => {
        unroll!(@$v, $a, 58, $c);
        unroll!(@$v, $a + 58, 58, $c);
    };

    (@$v:ident, $a:expr, 117, $c:block) => {
        unroll!(@$v, $a, 116, $c);
        { const $v: usize = $a + 116; $c }
    };

    (@$v:ident, $a:expr, 118, $c:block) => {
        unroll!(@$v, $a, 59, $c);
        unroll!(@$v, $a + 59, 59, $c);
    };

    (@$v:ident, $a:expr, 119, $c:block) => {
        unroll!(@$v, $a, 118, $c);
        { const $v: usize = $a + 118; $c }
    };

    (@$v:ident, $a:expr, 120, $c:block) => {
        unroll!(@$v, $a, 60, $c);
        unroll!(@$v, $a + 60, 60, $c);
    };

    (@$v:ident, $a:expr, 121, $c:block) => {
        unroll!(@$v, $a, 120, $c);
        { const $v: usize = $a + 120; $c }
    };

    (@$v:ident, $a:expr, 122, $c:block) => {
        unroll!(@$v, $a, 61, $c);
        unroll!(@$v, $a + 61, 61, $c);
    };

    (@$v:ident, $a:expr, 123, $c:block) => {
        unroll!(@$v, $a, 122, $c);
        { const $v: usize = $a + 122; $c }
    };

    (@$v:ident, $a:expr, 124, $c:block) => {
        unroll!(@$v, $a, 62, $c);
        unroll!(@$v, $a + 62, 62, $c);
    };

    (@$v:ident, $a:expr, 125, $c:block) => {
        unroll!(@$v, $a, 124, $c);
        { const $v: usize = $a + 124; $c }
    };

    (@$v:ident, $a:expr, 126, $c:block) => {
        unroll!(@$v, $a, 63, $c);
        unroll!(@$v, $a + 63, 63, $c);
    };

    (@$v:ident, $a:expr, 127, $c:block) => {
        unroll!(@$v, $a, 126, $c);
        { const $v: usize = $a + 126; $c }
    };

    (@$v:ident, $a:expr, 128, $c:block) => {
        unroll!(@$v, $a, 64, $c);
        unroll!(@$v, $a + 64, 64, $c);
    };

    (@$v:ident, $a:expr, 129, $c:block) => {
        unroll!(@$v, $a, 128, $c);
        { const $v: usize = $a + 128; $c }
    };

    (@$v:ident, $a:expr, 130, $c:block) => {
        unroll!(@$v, $a, 65, $c);
        unroll!(@$v, $a + 65, 65, $c);
    };

    (@$v:ident, $a:expr, 131, $c:block) => {
        unroll!(@$v, $a, 130, $c);
        { const $v: usize = $a + 130; $c }
    };

    (@$v:ident, $a:expr, 132, $c:block) => {
        unroll!(@$v, $a, 66, $c);
        unroll!(@$v, $a + 66, 66, $c);
    };

    (@$v:ident, $a:expr, 133, $c:block) => {
        unroll!(@$v, $a, 132, $c);
        { const $v: usize = $a + 132; $c }
    };

    (@$v:ident, $a:expr, 134, $c:block) => {
        unroll!(@$v, $a, 67, $c);
        unroll!(@$v, $a + 67, 67, $c);
    };

    (@$v:ident, $a:expr, 135, $c:block) => {
        unroll!(@$v, $a, 134, $c);
        { const $v: usize = $a + 134; $c }
    };

    (@$v:ident, $a:expr, 136, $c:block) => {
        unroll!(@$v, $a, 68, $c);
        unroll!(@$v, $a + 68, 68, $c);
    };

    (@$v:ident, $a:expr, 137, $c:block) => {
        unroll!(@$v, $a, 136, $c);
        { const $v: usize = $a + 136; $c }
    };

    (@$v:ident, $a:expr, 138, $c:block) => {
        unroll!(@$v, $a, 69, $c);
        unroll!(@$v, $a + 69, 69, $c);
    };

    (@$v:ident, $a:expr, 139, $c:block) => {
        unroll!(@$v, $a, 138, $c);
        { const $v: usize = $a + 138; $c }
    };

    (@$v:ident, $a:expr, 140, $c:block) => {
        unroll!(@$v, $a, 70, $c);
        unroll!(@$v, $a + 70, 70, $c);
    };

    (@$v:ident, $a:expr, 141, $c:block) => {
        unroll!(@$v, $a, 140, $c);
        { const $v: usize = $a + 140; $c }
    };

    (@$v:ident, $a:expr, 142, $c:block) => {
        unroll!(@$v, $a, 71, $c);
        unroll!(@$v, $a + 71, 71, $c);
    };

    (@$v:ident, $a:expr, 143, $c:block) => {
        unroll!(@$v, $a, 142, $c);
        { const $v: usize = $a + 142; $c }
    };

    (@$v:ident, $a:expr, 144, $c:block) => {
        unroll!(@$v, $a, 72, $c);
        unroll!(@$v, $a + 72, 72, $c);
    };

    (@$v:ident, $a:expr, 145, $c:block) => {
        unroll!(@$v, $a, 144, $c);
        { const $v: usize = $a + 144; $c }
    };

    (@$v:ident, $a:expr, 146, $c:block) => {
        unroll!(@$v, $a, 73, $c);
        unroll!(@$v, $a + 73, 73, $c);
    };

    (@$v:ident, $a:expr, 147, $c:block) => {
        unroll!(@$v, $a, 146, $c);
        { const $v: usize = $a + 146; $c }
    };

    (@$v:ident, $a:expr, 148, $c:block) => {
        unroll!(@$v, $a, 74, $c);
        unroll!(@$v, $a + 74, 74, $c);
    };

    (@$v:ident, $a:expr, 149, $c:block) => {
        unroll!(@$v, $a, 148, $c);
        { const $v: usize = $a + 148; $c }
    };

    (@$v:ident, $a:expr, 150, $c:block) => {
        unroll!(@$v, $a, 75, $c);
        unroll!(@$v, $a + 75, 75, $c);
    };

    (@$v:ident, $a:expr, 151, $c:block) => {
        unroll!(@$v, $a, 150, $c);
        { const $v: usize = $a + 150; $c }
    };

    (@$v:ident, $a:expr, 152, $c:block) => {
        unroll!(@$v, $a, 76, $c);
        unroll!(@$v, $a + 76, 76, $c);
    };

    (@$v:ident, $a:expr, 153, $c:block) => {
        unroll!(@$v, $a, 152, $c);
        { const $v: usize = $a + 152; $c }
    };

    (@$v:ident, $a:expr, 154, $c:block) => {
        unroll!(@$v, $a, 77, $c);
        unroll!(@$v, $a + 77, 77, $c);
    };

    (@$v:ident, $a:expr, 155, $c:block) => {
        unroll!(@$v, $a, 154, $c);
        { const $v: usize = $a + 154; $c }
    };

    (@$v:ident, $a:expr, 156, $c:block) => {
        unroll!(@$v, $a, 78, $c);
        unroll!(@$v, $a + 78, 78, $c);
    };

    (@$v:ident, $a:expr, 157, $c:block) => {
        unroll!(@$v, $a, 156, $c);
        { const $v: usize = $a + 156; $c }
    };

    (@$v:ident, $a:expr, 158, $c:block) => {
        unroll!(@$v, $a, 79, $c);
        unroll!(@$v, $a + 79, 79, $c);
    };

    (@$v:ident, $a:expr, 159, $c:block) => {
        unroll!(@$v, $a, 158, $c);
        { const $v: usize = $a + 158; $c }
    };

    (@$v:ident, $a:expr, 160, $c:block) => {
        unroll!(@$v, $a, 80, $c);
        unroll!(@$v, $a + 80, 80, $c);
    };

    (@$v:ident, $a:expr, 161, $c:block) => {
        unroll!(@$v, $a, 160, $c);
        { const $v: usize = $a + 160; $c }
    };

    (@$v:ident, $a:expr, 162, $c:block) => {
        unroll!(@$v, $a, 81, $c);
        unroll!(@$v, $a + 81, 81, $c);
    };

    (@$v:ident, $a:expr, 163, $c:block) => {
        unroll!(@$v, $a, 162, $c);
        { const $v: usize = $a + 162; $c }
    };

    (@$v:ident, $a:expr, 164, $c:block) => {
        unroll!(@$v, $a, 82, $c);
        unroll!(@$v, $a + 82, 82, $c);
    };

    (@$v:ident, $a:expr, 165, $c:block) => {
        unroll!(@$v, $a, 164, $c);
        { const $v: usize = $a + 164; $c }
    };

    (@$v:ident, $a:expr, 166, $c:block) => {
        unroll!(@$v, $a, 83, $c);
        unroll!(@$v, $a + 83, 83, $c);
    };

    (@$v:ident, $a:expr, 167, $c:block) => {
        unroll!(@$v, $a, 166, $c);
        { const $v: usize = $a + 166; $c }
    };

    (@$v:ident, $a:expr, 168, $c:block) => {
        unroll!(@$v, $a, 84, $c);
        unroll!(@$v, $a + 84, 84, $c);
    };

    (@$v:ident, $a:expr, 169, $c:block) => {
        unroll!(@$v, $a, 168, $c);
        { const $v: usize = $a + 168; $c }
    };

    (@$v:ident, $a:expr, 170, $c:block) => {
        unroll!(@$v, $a, 85, $c);
        unroll!(@$v, $a + 85, 85, $c);
    };

    (@$v:ident, $a:expr, 171, $c:block) => {
        unroll!(@$v, $a, 170, $c);
        { const $v: usize = $a + 170; $c }
    };

    (@$v:ident, $a:expr, 172, $c:block) => {
        unroll!(@$v, $a, 86, $c);
        unroll!(@$v, $a + 86, 86, $c);
    };

    (@$v:ident, $a:expr, 173, $c:block) => {
        unroll!(@$v, $a, 172, $c);
        { const $v: usize = $a + 172; $c }
    };

    (@$v:ident, $a:expr, 174, $c:block) => {
        unroll!(@$v, $a, 87, $c);
        unroll!(@$v, $a + 87, 87, $c);
    };

    (@$v:ident, $a:expr, 175, $c:block) => {
        unroll!(@$v, $a, 174, $c);
        { const $v: usize = $a + 174; $c }
    };

    (@$v:ident, $a:expr, 176, $c:block) => {
        unroll!(@$v, $a, 88, $c);
        unroll!(@$v, $a + 88, 88, $c);
    };

    (@$v:ident, $a:expr, 177, $c:block) => {
        unroll!(@$v, $a, 176, $c);
        { const $v: usize = $a + 176; $c }
    };

    (@$v:ident, $a:expr, 178, $c:block) => {
        unroll!(@$v, $a, 89, $c);
        unroll!(@$v, $a + 89, 89, $c);
    };

    (@$v:ident, $a:expr, 179, $c:block) => {
        unroll!(@$v, $a, 178, $c);
        { const $v: usize = $a + 178; $c }
    };

    (@$v:ident, $a:expr, 180, $c:block) => {
        unroll!(@$v, $a, 90, $c);
        unroll!(@$v, $a + 90, 90, $c);
    };

    (@$v:ident, $a:expr, 181, $c:block) => {
        unroll!(@$v, $a, 180, $c);
        { const $v: usize = $a + 180; $c }
    };

    (@$v:ident, $a:expr, 182, $c:block) => {
        unroll!(@$v, $a, 91, $c);
        unroll!(@$v, $a + 91, 91, $c);
    };

    (@$v:ident, $a:expr, 183, $c:block) => {
        unroll!(@$v, $a, 182, $c);
        { const $v: usize = $a + 182; $c }
    };

    (@$v:ident, $a:expr, 184, $c:block) => {
        unroll!(@$v, $a, 92, $c);
        unroll!(@$v, $a + 92, 92, $c);
    };

    (@$v:ident, $a:expr, 185, $c:block) => {
        unroll!(@$v, $a, 184, $c);
        { const $v: usize = $a + 184; $c }
    };

    (@$v:ident, $a:expr, 186, $c:block) => {
        unroll!(@$v, $a, 93, $c);
        unroll!(@$v, $a + 93, 93, $c);
    };

    (@$v:ident, $a:expr, 187, $c:block) => {
        unroll!(@$v, $a, 186, $c);
        { const $v: usize = $a + 186; $c }
    };

    (@$v:ident, $a:expr, 188, $c:block) => {
        unroll!(@$v, $a, 94, $c);
        unroll!(@$v, $a + 94, 94, $c);
    };

    (@$v:ident, $a:expr, 189, $c:block) => {
        unroll!(@$v, $a, 188, $c);
        { const $v: usize = $a + 188; $c }
    };

    (@$v:ident, $a:expr, 190, $c:block) => {
        unroll!(@$v, $a, 95, $c);
        unroll!(@$v, $a + 95, 95, $c);
    };

    (@$v:ident, $a:expr, 191, $c:block) => {
        unroll!(@$v, $a, 190, $c);
        { const $v: usize = $a + 190; $c }
    };

    (@$v:ident, $a:expr, 192, $c:block) => {
        unroll!(@$v, $a, 96, $c);
        unroll!(@$v, $a + 96, 96, $c);
    };

    (@$v:ident, $a:expr, 193, $c:block) => {
        unroll!(@$v, $a, 192, $c);
        { const $v: usize = $a + 192; $c }
    };

    (@$v:ident, $a:expr, 194, $c:block) => {
        unroll!(@$v, $a, 97, $c);
        unroll!(@$v, $a + 97, 97, $c);
    };

    (@$v:ident, $a:expr, 195, $c:block) => {
        unroll!(@$v, $a, 194, $c);
        { const $v: usize = $a + 194; $c }
    };

    (@$v:ident, $a:expr, 196, $c:block) => {
        unroll!(@$v, $a, 98, $c);
        unroll!(@$v, $a + 98, 98, $c);
    };

    (@$v:ident, $a:expr, 197, $c:block) => {
        unroll!(@$v, $a, 196, $c);
        { const $v: usize = $a + 196; $c }
    };

    (@$v:ident, $a:expr, 198, $c:block) => {
        unroll!(@$v, $a, 99, $c);
        unroll!(@$v, $a + 99, 99, $c);
    };

    (@$v:ident, $a:expr, 199, $c:block) => {
        unroll!(@$v, $a, 198, $c);
        { const $v: usize = $a + 198; $c }
    };

    (@$v:ident, $a:expr, 200, $c:block) => {
        unroll!(@$v, $a, 100, $c);
        unroll!(@$v, $a + 100, 100, $c);
    };

    (@$v:ident, $a:expr, 201, $c:block) => {
        unroll!(@$v, $a, 200, $c);
        { const $v: usize = $a + 200; $c }
    };

    (@$v:ident, $a:expr, 202, $c:block) => {
        unroll!(@$v, $a, 101, $c);
        unroll!(@$v, $a + 101, 101, $c);
    };

    (@$v:ident, $a:expr, 203, $c:block) => {
        unroll!(@$v, $a, 202, $c);
        { const $v: usize = $a + 202; $c }
    };

    (@$v:ident, $a:expr, 204, $c:block) => {
        unroll!(@$v, $a, 102, $c);
        unroll!(@$v, $a + 102, 102, $c);
    };

    (@$v:ident, $a:expr, 205, $c:block) => {
        unroll!(@$v, $a, 204, $c);
        { const $v: usize = $a + 204; $c }
    };

    (@$v:ident, $a:expr, 206, $c:block) => {
        unroll!(@$v, $a, 103, $c);
        unroll!(@$v, $a + 103, 103, $c);
    };

    (@$v:ident, $a:expr, 207, $c:block) => {
        unroll!(@$v, $a, 206, $c);
        { const $v: usize = $a + 206; $c }
    };

    (@$v:ident, $a:expr, 208, $c:block) => {
        unroll!(@$v, $a, 104, $c);
        unroll!(@$v, $a + 104, 104, $c);
    };

    (@$v:ident, $a:expr, 209, $c:block) => {
        unroll!(@$v, $a, 208, $c);
        { const $v: usize = $a + 208; $c }
    };

    (@$v:ident, $a:expr, 210, $c:block) => {
        unroll!(@$v, $a, 105, $c);
        unroll!(@$v, $a + 105, 105, $c);
    };

    (@$v:ident, $a:expr, 211, $c:block) => {
        unroll!(@$v, $a, 210, $c);
        { const $v: usize = $a + 210; $c }
    };

    (@$v:ident, $a:expr, 212, $c:block) => {
        unroll!(@$v, $a, 106, $c);
        unroll!(@$v, $a + 106, 106, $c);
    };

    (@$v:ident, $a:expr, 213, $c:block) => {
        unroll!(@$v, $a, 212, $c);
        { const $v: usize = $a + 212; $c }
    };

    (@$v:ident, $a:expr, 214, $c:block) => {
        unroll!(@$v, $a, 107, $c);
        unroll!(@$v, $a + 107, 107, $c);
    };

    (@$v:ident, $a:expr, 215, $c:block) => {
        unroll!(@$v, $a, 214, $c);
        { const $v: usize = $a + 214; $c }
    };

    (@$v:ident, $a:expr, 216, $c:block) => {
        unroll!(@$v, $a, 108, $c);
        unroll!(@$v, $a + 108, 108, $c);
    };

    (@$v:ident, $a:expr, 217, $c:block) => {
        unroll!(@$v, $a, 216, $c);
        { const $v: usize = $a + 216; $c }
    };

    (@$v:ident, $a:expr, 218, $c:block) => {
        unroll!(@$v, $a, 109, $c);
        unroll!(@$v, $a + 109, 109, $c);
    };

    (@$v:ident, $a:expr, 219, $c:block) => {
        unroll!(@$v, $a, 218, $c);
        { const $v: usize = $a + 218; $c }
    };

    (@$v:ident, $a:expr, 220, $c:block) => {
        unroll!(@$v, $a, 110, $c);
        unroll!(@$v, $a + 110, 110, $c);
    };

    (@$v:ident, $a:expr, 221, $c:block) => {
        unroll!(@$v, $a, 220, $c);
        { const $v: usize = $a + 220; $c }
    };

    (@$v:ident, $a:expr, 222, $c:block) => {
        unroll!(@$v, $a, 111, $c);
        unroll!(@$v, $a + 111, 111, $c);
    };

    (@$v:ident, $a:expr, 223, $c:block) => {
        unroll!(@$v, $a, 222, $c);
        { const $v: usize = $a + 222; $c }
    };

    (@$v:ident, $a:expr, 224, $c:block) => {
        unroll!(@$v, $a, 112, $c);
        unroll!(@$v, $a + 112, 112, $c);
    };

    (@$v:ident, $a:expr, 225, $c:block) => {
        unroll!(@$v, $a, 224, $c);
        { const $v: usize = $a + 224; $c }
    };

    (@$v:ident, $a:expr, 226, $c:block) => {
        unroll!(@$v, $a, 113, $c);
        unroll!(@$v, $a + 113, 113, $c);
    };

    (@$v:ident, $a:expr, 227, $c:block) => {
        unroll!(@$v, $a, 226, $c);
        { const $v: usize = $a + 226; $c }
    };

    (@$v:ident, $a:expr, 228, $c:block) => {
        unroll!(@$v, $a, 114, $c);
        unroll!(@$v, $a + 114, 114, $c);
    };

    (@$v:ident, $a:expr, 229, $c:block) => {
        unroll!(@$v, $a, 228, $c);
        { const $v: usize = $a + 228; $c }
    };

    (@$v:ident, $a:expr, 230, $c:block) => {
        unroll!(@$v, $a, 115, $c);
        unroll!(@$v, $a + 115, 115, $c);
    };

    (@$v:ident, $a:expr, 231, $c:block) => {
        unroll!(@$v, $a, 230, $c);
        { const $v: usize = $a + 230; $c }
    };

    (@$v:ident, $a:expr, 232, $c:block) => {
        unroll!(@$v, $a, 116, $c);
        unroll!(@$v, $a + 116, 116, $c);
    };

    (@$v:ident, $a:expr, 233, $c:block) => {
        unroll!(@$v, $a, 232, $c);
        { const $v: usize = $a + 232; $c }
    };

    (@$v:ident, $a:expr, 234, $c:block) => {
        unroll!(@$v, $a, 117, $c);
        unroll!(@$v, $a + 117, 117, $c);
    };

    (@$v:ident, $a:expr, 235, $c:block) => {
        unroll!(@$v, $a, 234, $c);
        { const $v: usize = $a + 234; $c }
    };

    (@$v:ident, $a:expr, 236, $c:block) => {
        unroll!(@$v, $a, 118, $c);
        unroll!(@$v, $a + 118, 118, $c);
    };

    (@$v:ident, $a:expr, 237, $c:block) => {
        unroll!(@$v, $a, 236, $c);
        { const $v: usize = $a + 236; $c }
    };

    (@$v:ident, $a:expr, 238, $c:block) => {
        unroll!(@$v, $a, 119, $c);
        unroll!(@$v, $a + 119, 119, $c);
    };

    (@$v:ident, $a:expr, 239, $c:block) => {
        unroll!(@$v, $a, 238, $c);
        { const $v: usize = $a + 238; $c }
    };

    (@$v:ident, $a:expr, 240, $c:block) => {
        unroll!(@$v, $a, 120, $c);
        unroll!(@$v, $a + 120, 120, $c);
    };

    (@$v:ident, $a:expr, 241, $c:block) => {
        unroll!(@$v, $a, 240, $c);
        { const $v: usize = $a + 240; $c }
    };

    (@$v:ident, $a:expr, 242, $c:block) => {
        unroll!(@$v, $a, 121, $c);
        unroll!(@$v, $a + 121, 121, $c);
    };

    (@$v:ident, $a:expr, 243, $c:block) => {
        unroll!(@$v, $a, 242, $c);
        { const $v: usize = $a + 242; $c }
    };

    (@$v:ident, $a:expr, 244, $c:block) => {
        unroll!(@$v, $a, 122, $c);
        unroll!(@$v, $a + 122, 122, $c);
    };

    (@$v:ident, $a:expr, 245, $c:block) => {
        unroll!(@$v, $a, 244, $c);
        { const $v: usize = $a + 244; $c }
    };

    (@$v:ident, $a:expr, 246, $c:block) => {
        unroll!(@$v, $a, 123, $c);
        unroll!(@$v, $a + 123, 123, $c);
    };

    (@$v:ident, $a:expr, 247, $c:block) => {
        unroll!(@$v, $a, 246, $c);
        { const $v: usize = $a + 246; $c }
    };

    (@$v:ident, $a:expr, 248, $c:block) => {
        unroll!(@$v, $a, 124, $c);
        unroll!(@$v, $a + 124, 124, $c);
    };

    (@$v:ident, $a:expr, 249, $c:block) => {
        unroll!(@$v, $a, 248, $c);
        { const $v: usize = $a + 248; $c }
    };

    (@$v:ident, $a:expr, 250, $c:block) => {
        unroll!(@$v, $a, 125, $c);
        unroll!(@$v, $a + 125, 125, $c);
    };

    (@$v:ident, $a:expr, 251, $c:block) => {
        unroll!(@$v, $a, 250, $c);
        { const $v: usize = $a + 250; $c }
    };

    (@$v:ident, $a:expr, 252, $c:block) => {
        unroll!(@$v, $a, 126, $c);
        unroll!(@$v, $a + 126, 126, $c);
    };

    (@$v:ident, $a:expr, 253, $c:block) => {
        unroll!(@$v, $a, 252, $c);
        { const $v: usize = $a + 252; $c }
    };

    (@$v:ident, $a:expr, 254, $c:block) => {
        unroll!(@$v, $a, 127, $c);
        unroll!(@$v, $a + 127, 127, $c);
    };

    (@$v:ident, $a:expr, 255, $c:block) => {
        unroll!(@$v, $a, 254, $c);
        { const $v: usize = $a + 254; $c }
    };

    (@$v:ident, $a:expr, 256, $c:block) => {
        unroll!(@$v, $a, 128, $c);
        unroll!(@$v, $a + 128, 128, $c);
    };

}


#[cfg(test)]
mod tests {
    #[test]
    fn test_all() {
        {
            let a: Vec<usize> = vec![];
            unroll! {
                for i in 0..0 {
                    a.push(i);
                }
            }
            assert_eq!(a, (0..0).collect::<Vec<usize>>());
        }
        {
            let mut a: Vec<usize> = vec![];
            unroll! {
                for i in 0..1 {
                    a.push(i);
                }
            }
            assert_eq!(a, (0..1).collect::<Vec<usize>>());
        }
        {
            let mut a: Vec<usize> = vec![];
            unroll! {
                for i in 0..256 {
                    a.push(i);
                }
            }
            assert_eq!(a, (0..256).collect::<Vec<usize>>());
        }
    }
}
