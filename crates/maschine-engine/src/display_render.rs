//! Rendu de texte et éléments UI pour les 2 écrans 480×272 de la MK3.

use maschine_proto::display::{DisplayFrame, DisplayId, DisplayPixel};

/// Police 8×16 bitmap simple pour ASCII 32..126
const FONT_8X16_BASIC: [[u8; 16]; 95] = [
    // 32 ' '
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 33 '!'
    [
        0x00, 0x00, 0x18, 0x3c, 0x3c, 0x3c, 0x18, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 34 '"'
    [
        0x00, 0x66, 0x66, 0x66, 0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 35 '#'
    [
        0x00, 0x00, 0x36, 0x36, 0x7f, 0x36, 0x36, 0x36, 0x7f, 0x36, 0x36, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 36 '$'
    [
        0x00, 0x18, 0x3e, 0x63, 0x60, 0x3e, 0x03, 0x06, 0x7c, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 37 '%'
    [
        0x00, 0x00, 0x61, 0x63, 0x36, 0x1c, 0x08, 0x38, 0x6c, 0x66, 0x46, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 38 '&'
    [
        0x00, 0x00, 0x1c, 0x36, 0x30, 0x1c, 0x36, 0x66, 0x6e, 0x3b, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 39 '\''
    [
        0x00, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 40 '('
    [
        0x00, 0x00, 0x0e, 0x1c, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1c, 0x0e, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 41 ')'
    [
        0x00, 0x00, 0x70, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x38, 0x70, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 42 '*'
    [
        0x00, 0x00, 0x00, 0x66, 0x3c, 0xff, 0x3c, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 43 '+'
    [
        0x00, 0x00, 0x00, 0x18, 0x18, 0x7e, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 44 ','
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x18, 0x30, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 45 '-'
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 46 '.'
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 47 '/'
    [
        0x00, 0x00, 0x03, 0x06, 0x0c, 0x18, 0x30, 0x60, 0xc0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 48 '0'
    [
        0x00, 0x00, 0x3c, 0x66, 0x6e, 0x76, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 49 '1'
    [
        0x00, 0x00, 0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 50 '2'
    [
        0x00, 0x00, 0x3c, 0x66, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 51 '3'
    [
        0x00, 0x00, 0x3c, 0x66, 0x06, 0x1c, 0x06, 0x06, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 52 '4'
    [
        0x00, 0x00, 0x0c, 0x1c, 0x3c, 0x6c, 0xfe, 0x0c, 0x0c, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 53 '5'
    [
        0x00, 0x00, 0x7e, 0x60, 0x7c, 0x06, 0x06, 0x06, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 54 '6'
    [
        0x00, 0x00, 0x1c, 0x30, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 55 '7'
    [
        0x00, 0x00, 0x7e, 0x66, 0x0c, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 56 '8'
    [
        0x00, 0x00, 0x3c, 0x66, 0x66, 0x3c, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 57 '9'
    [
        0x00, 0x00, 0x3c, 0x66, 0x66, 0x66, 0x3e, 0x06, 0x0c, 0x38, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 58 ':'
    [
        0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 59 ';'
    [
        0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x18, 0x30, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 60 '<'
    [
        0x00, 0x00, 0x0e, 0x1c, 0x38, 0x70, 0x38, 0x1c, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 61 '='
    [
        0x00, 0x00, 0x00, 0x7e, 0x00, 0x00, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 62 '>'
    [
        0x00, 0x00, 0x70, 0x38, 0x1c, 0x0e, 0x1c, 0x38, 0x70, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 63 '?'
    [
        0x00, 0x00, 0x3c, 0x66, 0x06, 0x0c, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 64 '@'
    [
        0x00, 0x00, 0x3c, 0x66, 0x6e, 0x6e, 0x60, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 65 'A'
    [
        0x00, 0x00, 0x18, 0x3c, 0x66, 0x66, 0x7e, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 66 'B'
    [
        0x00, 0x00, 0x7c, 0x66, 0x66, 0x7c, 0x66, 0x66, 0x66, 0x7c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 67 'C'
    [
        0x00, 0x00, 0x3c, 0x66, 0x60, 0x60, 0x60, 0x60, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 68 'D'
    [
        0x00, 0x00, 0x78, 0x6c, 0x66, 0x66, 0x66, 0x66, 0x6c, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 69 'E'
    [
        0x00, 0x00, 0x7e, 0x60, 0x60, 0x78, 0x60, 0x60, 0x60, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 70 'F'
    [
        0x00, 0x00, 0x7e, 0x60, 0x60, 0x78, 0x60, 0x60, 0x60, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 71 'G'
    [
        0x00, 0x00, 0x3c, 0x66, 0x60, 0x6e, 0x66, 0x66, 0x66, 0x3e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 72 'H'
    [
        0x00, 0x00, 0x66, 0x66, 0x66, 0x7e, 0x66, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 73 'I'
    [
        0x00, 0x00, 0x3e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 74 'J'
    [
        0x00, 0x00, 0x1e, 0x0c, 0x0c, 0x0c, 0x0c, 0x0c, 0x6c, 0x38, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 75 'K'
    [
        0x00, 0x00, 0x66, 0x6c, 0x78, 0x70, 0x78, 0x6c, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 76 'L'
    [
        0x00, 0x00, 0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 77 'M'
    [
        0x00, 0x00, 0x63, 0x77, 0x7f, 0x6b, 0x63, 0x63, 0x63, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 78 'N'
    [
        0x00, 0x00, 0x66, 0x76, 0x7e, 0x7e, 0x6e, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 79 'O'
    [
        0x00, 0x00, 0x3c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 80 'P'
    [
        0x00, 0x00, 0x7c, 0x66, 0x66, 0x7c, 0x60, 0x60, 0x60, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 81 'Q'
    [
        0x00, 0x00, 0x3c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x0e, 0x06, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 82 'R'
    [
        0x00, 0x00, 0x7c, 0x66, 0x66, 0x7c, 0x78, 0x6c, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 83 'S'
    [
        0x00, 0x00, 0x3c, 0x66, 0x60, 0x3c, 0x06, 0x06, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 84 'T'
    [
        0x00, 0x00, 0x7e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 85 'U'
    [
        0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 86 'V'
    [
        0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 87 'W'
    [
        0x00, 0x00, 0x63, 0x63, 0x63, 0x6b, 0x7f, 0x77, 0x63, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 88 'X'
    [
        0x00, 0x00, 0x66, 0x66, 0x3c, 0x18, 0x18, 0x3c, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 89 'Y'
    [
        0x00, 0x00, 0x66, 0x66, 0x66, 0x3c, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 90 'Z'
    [
        0x00, 0x00, 0x7e, 0x06, 0x0c, 0x18, 0x30, 0x60, 0x60, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 91 '['
    [
        0x00, 0x00, 0x1e, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 92 '\'
    [
        0x00, 0x00, 0x80, 0xc0, 0x60, 0x30, 0x18, 0x0c, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 93 ']'
    [
        0x00, 0x00, 0x78, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 94 '^'
    [
        0x00, 0x18, 0x3c, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 95 '_'
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 96 '`'
    [
        0x00, 0x30, 0x18, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 97 'a'
    [
        0x00, 0x00, 0x00, 0x00, 0x3c, 0x06, 0x3e, 0x66, 0x66, 0x3e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 98 'b'
    [
        0x00, 0x60, 0x60, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x66, 0x7c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 99 'c'
    [
        0x00, 0x00, 0x00, 0x00, 0x3c, 0x66, 0x60, 0x60, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 100 'd'
    [
        0x00, 0x06, 0x06, 0x06, 0x3e, 0x66, 0x66, 0x66, 0x66, 0x3e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 101 'e'
    [
        0x00, 0x00, 0x00, 0x00, 0x3c, 0x66, 0x7e, 0x60, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 102 'f'
    [
        0x00, 0x1c, 0x30, 0x30, 0x7c, 0x30, 0x30, 0x30, 0x30, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 103 'g'
    [
        0x00, 0x00, 0x00, 0x00, 0x3e, 0x66, 0x66, 0x3e, 0x06, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 104 'h'
    [
        0x00, 0x60, 0x60, 0x60, 0x7c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 105 'i'
    [
        0x00, 0x18, 0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 106 'j'
    [
        0x00, 0x0c, 0x0c, 0x00, 0x1c, 0x0c, 0x0c, 0x0c, 0x0c, 0x6c, 0x38, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 107 'k'
    [
        0x00, 0x60, 0x60, 0x60, 0x66, 0x6c, 0x78, 0x6c, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 108 'l'
    [
        0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 109 'm'
    [
        0x00, 0x00, 0x00, 0x00, 0x66, 0x7f, 0x7f, 0x6b, 0x63, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 110 'n'
    [
        0x00, 0x00, 0x00, 0x00, 0x7c, 0x66, 0x66, 0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 111 'o'
    [
        0x00, 0x00, 0x00, 0x00, 0x3c, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 112 'p'
    [
        0x00, 0x00, 0x00, 0x00, 0x7c, 0x66, 0x66, 0x7c, 0x60, 0x60, 0x60, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 113 'q'
    [
        0x00, 0x00, 0x00, 0x00, 0x3e, 0x66, 0x66, 0x3e, 0x06, 0x06, 0x0f, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 114 'r'
    [
        0x00, 0x00, 0x00, 0x00, 0x6c, 0x76, 0x60, 0x60, 0x60, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 115 's'
    [
        0x00, 0x00, 0x00, 0x00, 0x3e, 0x60, 0x3c, 0x06, 0x66, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 116 't'
    [
        0x00, 0x10, 0x30, 0x7c, 0x30, 0x30, 0x30, 0x30, 0x34, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 117 'u'
    [
        0x00, 0x00, 0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 118 'v'
    [
        0x00, 0x00, 0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x3c, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 119 'w'
    [
        0x00, 0x00, 0x00, 0x00, 0x63, 0x63, 0x6b, 0x7f, 0x36, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 120 'x'
    [
        0x00, 0x00, 0x00, 0x00, 0x66, 0x3c, 0x18, 0x18, 0x3c, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 121 'y'
    [
        0x00, 0x00, 0x00, 0x00, 0x66, 0x66, 0x66, 0x3e, 0x06, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 122 'z'
    [
        0x00, 0x00, 0x00, 0x00, 0x7e, 0x0c, 0x18, 0x30, 0x60, 0x7e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 123 '{'
    [
        0x00, 0x0e, 0x18, 0x18, 0x18, 0x70, 0x18, 0x18, 0x18, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 124 '|'
    [
        0x00, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 125 '}'
    [
        0x00, 0x70, 0x18, 0x18, 0x18, 0x0e, 0x18, 0x18, 0x18, 0x70, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
    // 126 '~'
    [
        0x00, 0x3b, 0x6e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ],
];

/// Dessine un caractère 8×16
pub fn draw_char(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    c: char,
    fg: DisplayPixel,
    bg: DisplayPixel,
) {
    let ascii = c as usize;
    let glyph = if (32..=126).contains(&ascii) {
        &FONT_8X16_BASIC[ascii - 32]
    } else {
        &FONT_8X16_BASIC[0]
    };

    let w = frame.w as usize;
    let h = frame.h as usize;

    for (row, &bits) in glyph.iter().enumerate() {
        let py = y + row;
        if py >= h {
            break;
        }
        for col in 0..8 {
            let px = x + col;
            if px >= w {
                break;
            }
            let is_set = (bits & (0x80 >> col)) != 0;
            let idx = py * w + px;
            if idx < frame.pixels.len() {
                frame.pixels[idx] = if is_set { fg } else { bg };
            }
        }
    }
}

/// Dessine du texte
pub fn draw_text(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    text: &str,
    fg: DisplayPixel,
    bg: DisplayPixel,
) {
    let mut cx = x;
    for c in text.chars() {
        draw_char(frame, cx, y, c, fg, bg);
        cx += 8;
    }
}

/// Remplis un rectangle
pub fn fill_rect(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: DisplayPixel,
) {
    let fw = frame.w as usize;
    let fh = frame.h as usize;
    for py in y..(y + h).min(fh) {
        for px in x..(x + w).min(fw) {
            let idx = py * fw + px;
            if idx < frame.pixels.len() {
                frame.pixels[idx] = color;
            }
        }
    }
}

/// Dessine le contour d'un rectangle.
pub fn draw_rect_outline(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: DisplayPixel,
) {
    if w == 0 || h == 0 {
        return;
    }
    fill_rect(frame, x, y, w, 1, color);
    fill_rect(frame, x, y + h - 1, w, 1, color);
    fill_rect(frame, x, y, 1, h, color);
    fill_rect(frame, x + w - 1, y, 1, h, color);
}

/// Dessine un onglet supérieur correspondant aux 4 boutons physiques.
pub fn draw_button_tab(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    label: &str,
    is_active: bool,
    active_color: DisplayPixel,
) {
    let bg = if is_active {
        DisplayPixel::from_rgb(38, 44, 60)
    } else {
        DisplayPixel::from_rgb(20, 22, 28)
    };
    let border = if is_active {
        active_color
    } else {
        DisplayPixel::from_rgb(36, 40, 50)
    };
    let text_col = if is_active {
        DisplayPixel::WHITE
    } else {
        DisplayPixel::from_rgb(130, 140, 155)
    };

    fill_rect(frame, x, y, w, h, bg);
    draw_rect_outline(frame, x, y, w, h, border);

    let text_len = label.len() * 8;
    let tx = if w > text_len {
        x + (w - text_len) / 2
    } else {
        x + 2
    };
    let ty = y + (h.saturating_sub(16)) / 2;
    draw_text(frame, tx, ty, label, text_col, bg);
}

/// Dessine un bloc paramètre pour un encodeur rotatif (y: 210..270).
pub fn draw_encoder_slot(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    name: &str,
    val_str: &str,
    val_norm: f32, // 0.0..1.0
    color: DisplayPixel,
) {
    let bg = DisplayPixel::from_rgb(22, 24, 32);
    let border = DisplayPixel::from_rgb(38, 42, 52);
    let text_gray = DisplayPixel::from_rgb(130, 140, 155);
    let white = DisplayPixel::WHITE;

    fill_rect(frame, x, y, w, h, bg);
    draw_rect_outline(frame, x, y, w, h, border);

    // Titre paramètre
    draw_text(frame, x + 6, y + 4, name, text_gray, bg);
    // Valeur paramètre
    draw_text(frame, x + 6, y + 22, val_str, white, bg);

    // Jauge de valeur horizontale en bas du slot
    let bar_y = y + h - 8;
    let bar_w = w.saturating_sub(12);
    fill_rect(
        frame,
        x + 6,
        bar_y,
        bar_w,
        4,
        DisplayPixel::from_rgb(36, 40, 52),
    );
    let fill = ((val_norm.clamp(0.0, 1.0) * bar_w as f32) as usize).min(bar_w);
    if fill > 0 {
        fill_rect(frame, x + 6, bar_y, fill, 4, color);
    }
}

/// Écran gauche officiel : Workflows, Groupe Focus, Grille des 16 Pads, Encodeurs 1..4.
pub fn render_daw_left(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Left, bg);

    let cyan = DisplayPixel::from_rgb(0, 200, 255);
    let white = DisplayPixel::WHITE;
    let _gray = DisplayPixel::from_rgb(120, 130, 150);

    // ── 1. Top Soft-Button Tabs [Boutons 1..4] ───────────────────────────
    let tabs = ["1:ARRANGE", "2:MIXER", "3:SAMPLER", "4:BROWSER"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        draw_button_tab(
            &mut frame,
            tx,
            2,
            114,
            22,
            tab,
            i == snap.left_tab as usize,
            cyan,
        );
    }

    // ── 2. Sous-titre Projet & Groupes A..H ─────────────────────────────
    let project_title = if snap.project.name.is_empty() {
        "Untitled"
    } else {
        &snap.project.name
    };
    // Tags de mode (pads = scènes/patterns/gomme/sélection + mode de jeu).
    let mode_tag = match snap.controller_mode {
        1 => " KEY",
        2 => " CHRD",
        3 => " STEP",
        _ => "",
    };
    let mut tags = String::new();
    if snap.scene_mode {
        tags.push_str(" SCENE");
    }
    if snap.pattern_mode {
        tags.push_str(" PAT");
    }
    if snap.select_mode {
        tags.push_str(" SEL");
    }
    if snap.erase_mode {
        tags.push_str(" ERASE");
    }
    tags.push_str(mode_tag);
    let name_short: String = project_title.chars().take(12).collect();
    let title_str = format!("PROJ:{}{}", name_short, tags);
    draw_text(&mut frame, 8, 28, &title_str, white, bg);

    // Groupes A..H en badges colorés
    for g in 0..8 {
        let gx = 180 + g * 36;
        let is_focus = g as u8 == snap.group_focus;
        let is_mute = (snap.muted_groups & (1 << g)) != 0;
        let is_solo = (snap.soloed_groups & (1 << g)) != 0;

        let g_col = if is_focus {
            cyan
        } else if is_mute {
            DisplayPixel::from_rgb(80, 40, 40)
        } else if is_solo {
            DisplayPixel::from_rgb(255, 200, 0)
        } else {
            DisplayPixel::from_rgb(32, 36, 46)
        };

        fill_rect(&mut frame, gx, 26, 32, 18, g_col);
        let g_char = (b'A' + g as u8) as char;
        let g_str = format!("{}", g_char);
        let txt_c = if is_focus {
            DisplayPixel::BLACK
        } else {
            DisplayPixel::WHITE
        };
        draw_text(&mut frame, gx + 12, 27, &g_str, txt_c, g_col);
    }

    // ── 3. Grille des 16 Pads (4×4 fidèle au contrôleur physique) ─────────
    // Disposition physique MK3 :
    // Ligne 0 : Pads 13..16 (index 12..15)
    // Ligne 1 : Pads  9..12 (index  8..11)
    // Ligne 2 : Pads  5..8  (index  4..7)
    // Ligne 3 : Pads  1..4  (index  0..3)
    let current_group = snap.project.groups.get(snap.group_focus as usize);

    for row in 0..4 {
        for col in 0..4 {
            let pad_num = (3 - row) * 4 + col + 1; // 1..16
            let pad_idx = pad_num - 1;

            let px = 8 + col * 116;
            let py = 48 + row * 38;
            let pw = 112;
            let ph = 34;

            let (name, pad_color) = if let Some(grp) = current_group {
                if let Some(snd) = grp.sounds.get(pad_idx).and_then(|s| s.as_ref()) {
                    let c = maschine_proto::color::Color::from_index(snd.color)
                        .map(|col| {
                            let (r, g, b) = col.0.to_rgb();
                            DisplayPixel::from_rgb(r, g, b)
                        })
                        .unwrap_or(cyan);
                    (snd.name.as_str(), c)
                } else {
                    ("-", DisplayPixel::from_rgb(45, 50, 60))
                }
            } else {
                ("-", DisplayPixel::from_rgb(45, 50, 60))
            };

            let cell_bg = DisplayPixel::from_rgb(22, 25, 34);
            fill_rect(&mut frame, px, py, pw, ph, cell_bg);
            draw_rect_outline(
                &mut frame,
                px,
                py,
                pw,
                ph,
                DisplayPixel::from_rgb(38, 42, 54),
            );

            // Pastille couleur du pad
            fill_rect(&mut frame, px + 4, py + 4, 6, ph - 8, pad_color);

            // Numéro Pad "01".."16"
            let num_str = format!("{:02}", pad_num);
            draw_text(&mut frame, px + 14, py + 8, &num_str, pad_color, cell_bg);

            // Nom du son tronqué
            let trunc_name: String = name.chars().take(8).collect();
            draw_text(&mut frame, px + 36, py + 8, &trunc_name, white, cell_bg);
        }
    }

    // ── 4. Bottom Encoder Parameter Slots [Encodeurs 1..4] ────────────────
    let bpm_str = format!("{:.1}", snap.transport.bpm);
    let swing_str = format!("{:.0}%", snap.project.swing * 100.0);
    let vol_str = format!("{:.1} dB", (snap.master_volume * 12.0) - 12.0);

    draw_encoder_slot(
        &mut frame,
        8,
        208,
        112,
        60,
        "1:TEMPO",
        &bpm_str,
        (snap.transport.bpm - 40.0) / 200.0,
        cyan,
    );
    draw_encoder_slot(
        &mut frame,
        124,
        208,
        112,
        60,
        "2:SWING",
        &swing_str,
        snap.project.swing,
        cyan,
    );
    draw_encoder_slot(
        &mut frame,
        240,
        208,
        112,
        60,
        "3:GRP VOL",
        &vol_str,
        snap.master_volume,
        cyan,
    );
    draw_encoder_slot(
        &mut frame,
        356,
        208,
        112,
        60,
        "4:GRP PAN",
        "CENTER",
        0.5,
        cyan,
    );

    frame
}

/// Écran droit officiel : Transport BBT, Séquenceur 16 pas, VU-Mètres Stéréo, Encodeurs 5..8.
pub fn render_daw_right(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Right, bg);

    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let green = DisplayPixel::from_rgb(0, 220, 100);
    let red = DisplayPixel::from_rgb(255, 50, 50);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);

    // ── 1. Top Soft-Button Tabs [Boutons 5..8] ───────────────────────────
    let tabs = ["5:BAR 1", "6:BAR 2", "7:BAR 3", "8:BAR 4"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        let is_sel = i == snap.selected_bar;
        let is_avail = i < snap.pattern_bars;
        let tab_col = if is_sel {
            yellow
        } else if is_avail {
            white
        } else {
            gray
        };
        draw_button_tab(
            &mut frame,
            tx,
            2,
            114,
            22,
            tab,
            is_sel,
            tab_col,
        );
    }

    // ── 2. Transport State Badge & Position BBT ──────────────────────────
    let (state_str, state_color) = match snap.transport.state {
        crate::transport::TransportState::Playing => ("PLAY", green),
        crate::transport::TransportState::Recording => ("REC", red),
        crate::transport::TransportState::Stopped => ("STOP", gray),
    };
    fill_rect(&mut frame, 8, 28, 48, 18, state_color);
    let st_txt_col = if snap.transport.state == crate::transport::TransportState::Playing {
        DisplayPixel::BLACK
    } else {
        white
    };
    draw_text(&mut frame, 12, 29, state_str, st_txt_col, state_color);

    // Position BBT grand format avec rappel de la mesure affichée
    let disp_bar = snap.selected_bar;
    let total_bars = snap.pattern_bars.max(1);
    let bbt_str = format!(
        "BAR {:02}:{:02}:{:03} [EDIT {}/{}]",
        snap.bbt.bars, snap.bbt.beats, snap.bbt.ticks, disp_bar + 1, total_bars
    );
    draw_text(&mut frame, 68, 29, &bbt_str, white, bg);

    // Signature temporelle & Voix actives + LOOP/METRO
    let loop_txt = if snap.transport.loop_enabled { " LOOP" } else { "" };
    let met_txt = if snap.transport.metronome { " MET" } else { "" };
    let info_str = format!("4/4 VOI:{}{}{}", snap.num_voices, loop_txt, met_txt);
    draw_text(&mut frame, 340, 29, &info_str, gray, bg);

    // ── 3. Step Sequencer 16 Pas avec Playhead dynamique ─────────────────
    // 4 pistes rythmiques (Kick, Snare, HH Cl, HH Op)
    let tracks = ["KICK ", "SNARE", "HH-CL", "HH-OP"];
    let pad_map: [u8; 4] = [0, 1, 2, 3];
    let current_step = ((snap.bbt.beats as u32 * 4 + (snap.bbt.ticks / 240) as u32) % 16) as usize;
    let playing_bar = (snap.bbt.bars.saturating_sub(1) as usize) % total_bars;
    let is_current_bar_playing = playing_bar == disp_bar
        && snap.transport.state == crate::transport::TransportState::Playing;

    let active_pattern = snap
        .project
        .active_pattern_index(snap.group_focus as usize, snap.active_scene)
        .and_then(|pi| {
            snap.project
                .groups
                .get(snap.group_focus as usize)?
                .patterns
                .get(pi)
        });

    for (t, &track_name) in tracks.iter().enumerate() {
        let ty = 54 + t * 24;
        draw_text(&mut frame, 8, ty + 3, track_name, gray, bg);

        let pad_idx = pad_map[t];

        for step in 0..16 {
            let sx = 64 + step * 25;
            let sw = 22;
            let sh = 20;

            // Détection si un trigger est programmé sur ce pas dans la mesure sélectionnée
            // (comparaison par case de 16e : tolère humanize/nudge, cf. toggle_step)
            let tick = (disp_bar as u32 * 4 * 960) + (step as u32 * 240);
            let is_triggered = if let Some(pat) = active_pattern {
                pat.events
                    .iter()
                    .any(|e| e.pad == pad_idx && crate::project::same_step_cell(e.tick, tick))
            } else {
                false
            };

            let is_playhead = step == current_step && is_current_bar_playing;

            let cell_col = if is_playhead {
                white
            } else if is_triggered {
                yellow
            } else if step % 4 == 0 {
                DisplayPixel::from_rgb(34, 38, 50)
            } else {
                DisplayPixel::from_rgb(22, 25, 32)
            };

            fill_rect(&mut frame, sx, ty, sw, sh, cell_col);
            draw_rect_outline(
                &mut frame,
                sx,
                ty,
                sw,
                sh,
                DisplayPixel::from_rgb(45, 50, 65),
            );
        }
    }

    // ── 4. Double VU-Mètre Stéréo Master L / R ───────────────────────────
    draw_text(&mut frame, 8, 156, "MASTER OUT L/R", gray, bg);

    let vu_w = 460;
    let vu_level_l = if snap.transport.state == crate::transport::TransportState::Playing {
        (snap.master_volume * 0.85).clamp(0.0, 1.0)
    } else {
        0.05
    };
    let vu_level_r = if snap.transport.state == crate::transport::TransportState::Playing {
        (snap.master_volume * 0.80).clamp(0.0, 1.0)
    } else {
        0.05
    };

    // Canal L
    fill_rect(
        &mut frame,
        8,
        174,
        vu_w,
        8,
        DisplayPixel::from_rgb(24, 28, 36),
    );
    let fill_l = ((vu_level_l * vu_w as f32) as usize).min(vu_w);
    for x in 0..fill_l {
        let col = if x < (vu_w * 7 / 10) {
            green
        } else if x < (vu_w * 9 / 10) {
            yellow
        } else {
            red
        };
        fill_rect(&mut frame, 8 + x, 174, 1, 8, col);
    }

    // Canal R
    fill_rect(
        &mut frame,
        8,
        186,
        vu_w,
        8,
        DisplayPixel::from_rgb(24, 28, 36),
    );
    let fill_r = ((vu_level_r * vu_w as f32) as usize).min(vu_w);
    for x in 0..fill_r {
        let col = if x < (vu_w * 7 / 10) {
            green
        } else if x < (vu_w * 9 / 10) {
            yellow
        } else {
            red
        };
        fill_rect(&mut frame, 8 + x, 186, 1, 8, col);
    }

    // ── 5. Bottom Encoder Parameter Slots [Encodeurs 5..8] ────────────────
    let master_pct = format!("{:.0}%", snap.master_volume * 100.0);
    draw_encoder_slot(
        &mut frame, 8, 208, 112, 60, "5:TUNE", "+0.00 st", 0.5, yellow,
    );
    draw_encoder_slot(
        &mut frame, 124, 208, 112, 60, "6:CUTOFF", "20.0 kHz", 0.95, yellow,
    );
    draw_encoder_slot(&mut frame, 240, 208, 112, 60, "7:RESO", "0 %", 0.0, yellow);
    draw_encoder_slot(
        &mut frame,
        356,
        208,
        112,
        60,
        "8:MASTER",
        &master_pct,
        snap.master_volume,
        yellow,
    );

    // ── Infos Sound sélectionné ─────────────────────────────────────────
    let (snd_name, snd_vol, snd_pan, snd_cut) = snap
        .project
        .groups
        .get(snap.group_focus as usize)
        .and_then(|g| {
            g.sounds
                .get(snap.active_sound as usize)
                .and_then(|s| s.as_ref())
        })
        .map(|s| (s.name.as_str(), s.volume_db, s.pan, s.filter.cutoff_hz))
        .unwrap_or(("-", 0.0, 0.0, 1000.0));
    let sel_str = format!(
        "SOUND {:02}: {}  VOL {:+.1}dB  PAN {:+.2}  CUTOFF {:.0}Hz",
        snap.active_sound + 1,
        snd_name,
        snd_vol,
        snd_pan,
        snd_cut
    );
    draw_text(&mut frame, 8, 196, &sel_str, yellow, bg);

    frame
}

/// Rendu rétro-compatible écran gauche
pub fn render_status_left(project_name: &str, group_focus: u8, bpm: f32) -> DisplayFrame {
    let snap = crate::engine::EngineSnapshot {
        project: crate::project::Project {
            name: project_name.to_string(),
            ..Default::default()
        },
        transport: crate::transport::Transport::new(bpm, 48000),
        group_focus,
        active_scene: None,
        bbt: crate::project::BarsBeatsTicks::new(1, 0, 0),
        num_voices: 0,
        master_volume: 1.0,
        headphone_volume: 0.8,
        muted_groups: 0,
        soloed_groups: 0,
        left_tab: 0,
        right_tab: 0,
        active_sound: 0,
        song_mode: false,
        active_section: None,
        triggered_pads: [false; 16],
        scene_mode: false,
        pattern_mode: false,
        select_mode: false,
        erase_mode: false,
        controller_mode: 0,
        browser_preset_idx: 0,
        selected_bar: 0,
        pattern_bars: 2,
    };
    render_daw_left(&snap)
}

/// Rendu rétro-compatible écran droit
pub fn render_status_right(
    _state_str: &str,
    _bbt_str: &str,
    num_voices: usize,
    master_vol: f32,
) -> DisplayFrame {
    let snap = crate::engine::EngineSnapshot {
        project: crate::project::Project::new_demo(),
        transport: crate::transport::Transport::new(120.0, 48000),
        group_focus: 0,
        active_scene: None,
        bbt: crate::project::BarsBeatsTicks::new(1, 0, 0),
        num_voices,
        master_volume: master_vol,
        headphone_volume: 0.8,
        muted_groups: 0,
        soloed_groups: 0,
        left_tab: 0,
        right_tab: 0,
        active_sound: 0,
        song_mode: false,
        active_section: None,
        triggered_pads: [false; 16],
        scene_mode: false,
        pattern_mode: false,
        select_mode: false,
        erase_mode: false,
        controller_mode: 0,
        browser_preset_idx: 0,
        selected_bar: 0,
        pattern_bars: 2,
    };
    render_daw_right(&snap)
}

/// Écran gauche — vue Arranger : sections du Song et scènes
pub fn render_arranger_left(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    use maschine_proto::display::{DisplayFrame, DisplayId, DisplayPixel};
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Left, bg);
    let cyan = DisplayPixel::from_rgb(0, 200, 255);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let green = DisplayPixel::from_rgb(0, 220, 100);

    let title = "ARRANGER / SONG";
    draw_text(&mut frame, 8, 4, title, cyan, bg);

    // Transport
    let pos = snap.bbt;
    let state = if snap.transport.is_playing() {
        "PLAY"
    } else {
        "STOP"
    };
    let transport_str = format!(
        "{}  BBT {}:{}:{}  BPM {:.1}",
        state, pos.bars, pos.beats, pos.ticks, snap.transport.bpm
    );
    draw_text(&mut frame, 8, 24, &transport_str, white, bg);

    let mut y = 48;
    if snap.project.song.is_empty() {
        draw_text(&mut frame, 8, y, "Aucune section (Song vide)", gray, bg);
    } else {
        draw_text(&mut frame, 8, y, "SECTIONS:", yellow, bg);
        y += 18;
        for (i, sec) in snap.project.song.iter().enumerate() {
            let is_active = snap.active_section == Some(i)
                || (!snap.song_mode && snap.active_scene == Some(sec.scene));
            let color = if is_active { green } else { white };
            let scene_name = snap
                .project
                .scenes
                .get(sec.scene)
                .map(|s| s.name.as_str())
                .unwrap_or("?");
            let line = format!(
                "{:02}. {}  [{}]  {} bars",
                i + 1,
                sec.name,
                scene_name,
                sec.length.bars
            );
            draw_text(&mut frame, 12, y, &line, color, bg);
            if is_active {
                fill_rect(
                    &mut frame,
                    4,
                    y - 2,
                    472,
                    16,
                    DisplayPixel::from_rgb(30, 60, 40),
                );
                // redraw text on top
                draw_text(
                    &mut frame,
                    12,
                    y,
                    &line,
                    green,
                    DisplayPixel::from_rgb(30, 60, 40),
                );
            }
            y += 18;
            if y > 230 {
                break;
            }
        }
    }

    // Total length
    let total_ticks: u32 = snap
        .project
        .song
        .iter()
        .map(|s| s.length.to_ticks())
        .sum::<u32>();
    let total_str = format!(
        "Duree totale: {} ticks  ({} sec)",
        total_ticks,
        snap.transport.tick_to_secs(total_ticks as u64)
    );
    draw_text(&mut frame, 8, 238, &total_str, gray, bg);

    // Groups status footer
    let gf_str = format!(
        "Group focus: {}  Master {:.0}%",
        (b'A' + snap.group_focus) as char,
        snap.master_volume * 100.0
    );
    draw_text(&mut frame, 8, 256, &gf_str, cyan, bg);

    frame
}

/// Écran droit — vue Arranger : timeline sections et playhead
pub fn render_arranger_right(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    use maschine_proto::display::{DisplayFrame, DisplayId, DisplayPixel};
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Right, bg);
    let cyan = DisplayPixel::from_rgb(0, 200, 255);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let green = DisplayPixel::from_rgb(0, 220, 100);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);

    draw_text(&mut frame, 8, 4, "TIMELINE / PLAYHEAD", cyan, bg);

    // Timeline bar
    let total_ticks: u32 = snap
        .project
        .song
        .iter()
        .map(|s| s.length.to_ticks())
        .sum::<u32>()
        .max(1);
    let mut x = 8usize;
    for (i, sec) in snap.project.song.iter().enumerate() {
        let w = ((sec.length.to_ticks() as f32 / total_ticks as f32) * 464.0).round() as usize;
        let col = if snap.active_section == Some(i) {
            green
        } else {
            DisplayPixel::from_rgb(40, 50, 65)
        };
        fill_rect(&mut frame, x, 40, w.max(4), 40, col);
        draw_rect_outline(&mut frame, x, 40, w.max(4), 40, gray);
        let lbl = format!("{}", i + 1);
        draw_text(&mut frame, x + 4, 52, &lbl, white, col);
        x += w;
        if x > 472 {
            break;
        }
    }

    // Playhead
    if total_ticks > 0 {
        let frac = (snap.transport.tick % (total_ticks as u64)) as f32 / total_ticks as f32;
        let px = 8 + (frac * 464.0) as usize;
        fill_rect(&mut frame, px, 30, 2, 60, yellow);
    }

    // Section detail
    let mut y = 96;
    draw_text(&mut frame, 8, y, "SECTIONS:", yellow, bg);
    y += 18;
    for (i, sec) in snap.project.song.iter().enumerate() {
        let line = format!(
            "{:02} {}  scene:{}  len:{}",
            i + 1,
            sec.name,
            sec.scene,
            sec.length
        );
        let col = if snap.active_section == Some(i) {
            green
        } else {
            white
        };
        draw_text(&mut frame, 12, y, &line, col, bg);
        y += 16;
        if y > 250 {
            break;
        }
    }

    // Current scene info
    let scene_info = snap
        .active_scene
        .and_then(|si| snap.project.scenes.get(si))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "-".to_string());
    let info = format!(
        "Scene active: {}  |  Group: {}",
        scene_info,
        (b'A' + snap.group_focus) as char
    );
    draw_text(&mut frame, 8, 260, &info, cyan, bg);

    frame
}

/// Écran gauche — vue SAMPLER : édition du Sound/Pad sélectionné (style Maschine)
/// Accès hardware : bouton SAMPLING -> left_tab=2. Pads 1..16 sélectionnent le Sound.
pub fn render_sampler_left(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Left, bg);
    let cyan = DisplayPixel::from_rgb(0, 200, 255);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let green = DisplayPixel::from_rgb(0, 220, 100);

    // Tabs haut — met SAMPLER en surbrillance
    let tabs = ["1:ARRANGE", "2:MIXER", "3:SAMPLER", "4:BROWSER"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        draw_button_tab(&mut frame, tx, 2, 114, 22, tab, i == 2, cyan);
    }

    let group_char = (b'A' + snap.group_focus) as char;
    let snd_idx = snap.active_sound;
    let snd = snap
        .project
        .groups
        .get(snap.group_focus as usize)
        .and_then(|g| g.sounds.get(snd_idx as usize).and_then(|s| s.as_ref()));

    let (name, vol, pan, cutoff, reso) = snd
        .map(|s| {
            (
                s.name.as_str(),
                s.volume_db,
                s.pan,
                s.filter.cutoff_hz,
                s.filter.resonance,
            )
        })
        .unwrap_or(("-", 0.0, 0.0, 20000.0, 0.0));

    // Header Groupe / Sound — gros et lisible
    let header = format!("GRP {}  >  PAD {:02} : {}", group_char, snd_idx + 1, name);
    draw_text(&mut frame, 8, 28, &header, yellow, bg);
    let sub = "Tapez un PAD 1..16 pour selectionner le son a editer";
    draw_text(&mut frame, 8, 46, sub, gray, bg);

    // Ligne params principaux
    let pan_txt = if pan.abs() < 0.05 {
        "C".to_string()
    } else if pan < 0.0 {
        format!("L{:.0}", -pan * 100.0)
    } else {
        format!("R{:.0}", pan * 100.0)
    };
    let line1 = format!(
        "VOL {:+.1}dB   PAN {}   CUTOFF {:.0}Hz   RESO {:.0}%",
        vol,
        pan_txt,
        cutoff,
        reso * 100.0
    );
    draw_text(&mut frame, 8, 68, &line1, white, bg);

    // Mini grille 16 pads avec sélection visible (4x4 compacte)
    for row in 0..4usize {
        for col in 0..4usize {
            let pad_num = (3 - row) * 4 + col;
            let px = 8 + col * 116;
            let py = 92 + row * 26;
            let is_sel = pad_num as u8 == snd_idx;
            let cell = if is_sel {
                DisplayPixel::from_rgb(50, 70, 60)
            } else {
                DisplayPixel::from_rgb(22, 25, 34)
            };
            fill_rect(&mut frame, px, py, 112, 22, cell);
            let border = if is_sel {
                green
            } else {
                DisplayPixel::from_rgb(38, 42, 54)
            };
            draw_rect_outline(&mut frame, px, py, 112, 22, border);
            let label = format!("{:02}", pad_num + 1);
            draw_text(
                &mut frame,
                px + 4,
                py + 3,
                &label,
                if is_sel { green } else { gray },
                cell,
            );
            if let Some(nm) = snap
                .project
                .groups
                .get(snap.group_focus as usize)
                .and_then(|g| g.sounds.get(pad_num).and_then(|s| s.as_ref()))
                .map(|s| s.name.as_str())
            {
                let trunc: String = nm.chars().take(9).collect();
                draw_text(&mut frame, px + 28, py + 3, &trunc, white, cell);
            }
        }
    }

    // Slots encodeurs 1..4 -> VOL / PAN / CUTOFF / RESO du Sound actif
    let vol_norm = ((vol + 60.0) / 72.0).clamp(0.0, 1.0);
    let pan_norm = ((pan + 1.0) / 2.0).clamp(0.0, 1.0);
    // cutoff log 20..20k
    let cut_norm = ((cutoff.ln() - 20f32.ln()) / (20000f32.ln() - 20f32.ln())).clamp(0.0, 1.0);
    draw_encoder_slot(
        &mut frame,
        8,
        208,
        112,
        60,
        "1:S-VOL",
        &format!("{:+.1}dB", vol),
        vol_norm,
        cyan,
    );
    draw_encoder_slot(
        &mut frame, 124, 208, 112, 60, "2:S-PAN", &pan_txt, pan_norm, cyan,
    );
    draw_encoder_slot(
        &mut frame,
        240,
        208,
        112,
        60,
        "3:CUTOFF",
        &format!("{:.0}Hz", cutoff),
        cut_norm,
        cyan,
    );
    draw_encoder_slot(
        &mut frame,
        356,
        208,
        112,
        60,
        "4:RESO",
        &format!("{:.0}%", reso * 100.0),
        reso,
        cyan,
    );

    frame
}

/// Écran droit — vue SAMPLER : enveloppe ADSR + zone/sample du Sound actif
/// Encodeurs 5..8 -> ATTACK / DECAY / SUSTAIN / RELEASE
pub fn render_sampler_right(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Right, bg);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let green = DisplayPixel::from_rgb(0, 220, 100);

    let tabs = ["5:PATTERN", "6:EVENTS", "7:MACRO", "8:SETTINGS"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        draw_button_tab(&mut frame, tx, 2, 114, 22, tab, false, yellow);
    }

    let snd_idx = snap.active_sound;
    let snd = snap
        .project
        .groups
        .get(snap.group_focus as usize)
        .and_then(|g| g.sounds.get(snd_idx as usize).and_then(|s| s.as_ref()));

    let (name, a, d, s, r) = snd
        .map(|x| {
            (
                x.name.as_str(),
                x.adsr.attack_ms,
                x.adsr.decay_ms,
                x.adsr.sustain,
                x.adsr.release_ms,
            )
        })
        .unwrap_or(("-", 2.0, 100.0, 0.8, 150.0));
    let header = format!("ENV PAD {:02} : {}", snd_idx + 1, name);
    draw_text(&mut frame, 8, 28, &header, yellow, bg);

    // Barres ADSR simplifiées
    let bar_w = 440usize;
    let ay = 52usize;
    // Attack 0..500ms, Decay 0..1000, Sustain 0..1, Release 0..1000
    let a_n = (a / 500.0).clamp(0.0, 1.0);
    let d_n = (d / 1000.0).clamp(0.0, 1.0);
    let r_n = (r / 1000.0).clamp(0.0, 1.0);
    draw_text(&mut frame, 8, ay, &format!("ATK {:.1}ms", a), white, bg);
    fill_rect(
        &mut frame,
        140,
        ay,
        bar_w - 140,
        10,
        DisplayPixel::from_rgb(24, 28, 36),
    );
    fill_rect(
        &mut frame,
        140,
        ay,
        ((bar_w - 140) as f32 * a_n) as usize,
        10,
        green,
    );
    draw_text(
        &mut frame,
        8,
        ay + 16,
        &format!("DEC {:.0}ms", d),
        white,
        bg,
    );
    fill_rect(
        &mut frame,
        140,
        ay + 16,
        bar_w - 140,
        10,
        DisplayPixel::from_rgb(24, 28, 36),
    );
    fill_rect(
        &mut frame,
        140,
        ay + 16,
        ((bar_w - 140) as f32 * d_n) as usize,
        10,
        green,
    );
    draw_text(
        &mut frame,
        8,
        ay + 32,
        &format!("SUS {:.0}%", s * 100.0),
        white,
        bg,
    );
    fill_rect(
        &mut frame,
        140,
        ay + 32,
        bar_w - 140,
        10,
        DisplayPixel::from_rgb(24, 28, 36),
    );
    fill_rect(
        &mut frame,
        140,
        ay + 32,
        ((bar_w - 140) as f32 * s) as usize,
        10,
        green,
    );
    draw_text(
        &mut frame,
        8,
        ay + 48,
        &format!("REL {:.0}ms", r),
        white,
        bg,
    );
    fill_rect(
        &mut frame,
        140,
        ay + 48,
        bar_w - 140,
        10,
        DisplayPixel::from_rgb(24, 28, 36),
    );
    fill_rect(
        &mut frame,
        140,
        ay + 48,
        ((bar_w - 140) as f32 * r_n) as usize,
        10,
        green,
    );

    // Infos zone / choke
    let (choke_txt, sample_txt) = snd
        .map(|x| {
            (
                x.choke_group
                    .map(|c| format!("CHOKE {}", c))
                    .unwrap_or_else(|| "CHOKE -".to_string()),
                x.zones
                    .first()
                    .and_then(|z| z.sample_path.clone())
                    .unwrap_or_else(|| "(synth 808)".to_string()),
            )
        })
        .unwrap_or(("CHOKE -".to_string(), "-".to_string()));
    draw_text(
        &mut frame,
        8,
        132,
        &format!("{}   SAMPLE: {}", choke_txt, sample_txt),
        gray,
        bg,
    );
    draw_text(
        &mut frame,
        8,
        150,
        "Knobs 5..8 = ATK/DEC/SUS/REL du son actif",
        gray,
        bg,
    );

    // Slots encodeurs 5..8 -> ADSR
    draw_encoder_slot(
        &mut frame,
        8,
        208,
        112,
        60,
        "5:ATTACK",
        &format!("{:.1}ms", a),
        a_n,
        yellow,
    );
    draw_encoder_slot(
        &mut frame,
        124,
        208,
        112,
        60,
        "6:DECAY",
        &format!("{:.0}ms", d),
        d_n,
        yellow,
    );
    draw_encoder_slot(
        &mut frame,
        240,
        208,
        112,
        60,
        "7:SUSTAIN",
        &format!("{:.0}%", s * 100.0),
        s,
        yellow,
    );
    draw_encoder_slot(
        &mut frame,
        356,
        208,
        112,
        60,
        "8:RELEASE",
        &format!("{:.0}ms", r),
        r_n,
        yellow,
    );

    frame
}

/// Écran gauche — vue MIXER : volumes des 8 groupes A..H (knobs 1..8 = VOL A..H).
pub fn render_mixer_left(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Left, bg);
    let cyan = DisplayPixel::from_rgb(0, 200, 255);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let green = DisplayPixel::from_rgb(0, 220, 100);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let red = DisplayPixel::from_rgb(255, 50, 50);

    let tabs = ["1:ARRANGE", "2:MIXER", "3:SAMPLER", "4:BROWSER"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        draw_button_tab(&mut frame, tx, 2, 114, 22, tab, i == 1, cyan);
    }
    draw_text(&mut frame, 8, 28, "MIXER - VOL A..H (knobs 1..8)", yellow, bg);

    // 8 tranches (56px chacune).
    for g in 0..8usize {
        let gx = 8 + g * 58;
        let is_focus = g as u8 == snap.group_focus;
        let (vol, muted, soloed, name) = snap
            .project
            .groups
            .get(g)
            .map(|gr| (gr.volume_db, gr.muted, (snap.soloed_groups & (1 << g)) != 0, gr.name.as_str()))
            .unwrap_or((0.0, false, false, "-"));
        let eff_muted = muted || (snap.muted_groups & (1 << g)) != 0;
        // Fond tranche
        let cell = if is_focus {
            DisplayPixel::from_rgb(30, 50, 70)
        } else {
            DisplayPixel::from_rgb(22, 25, 34)
        };
        fill_rect(&mut frame, gx, 48, 52, 150, cell);
        draw_rect_outline(
            &mut frame,
            gx,
            48,
            52,
            150,
            if is_focus { cyan } else { DisplayPixel::from_rgb(38, 42, 54) },
        );
        // Lettre groupe
        let g_char = (b'A' + g as u8) as char;
        draw_text(&mut frame, gx + 4, 52, &format!("{}", g_char), if is_focus { cyan } else { white }, cell);
        // M / S
        if eff_muted {
            draw_text(&mut frame, gx + 22, 52, "M", red, cell);
        }
        if soloed {
            draw_text(&mut frame, gx + 34, 52, "S", yellow, cell);
        }
        // Barre verticale : -60..+12 dB -> 0..130px
        let norm = ((vol + 60.0) / 72.0).clamp(0.0, 1.0);
        let bar_h = (norm * 110.0) as usize;
        let bx = gx + 16;
        fill_rect(&mut frame, bx, 72, 20, 110, DisplayPixel::from_rgb(24, 28, 36));
        if bar_h > 0 {
            let col = if eff_muted { gray } else { green };
            fill_rect(&mut frame, bx, 72 + 110 - bar_h, 20, bar_h, col);
        }
        // Valeur dB
        let db_str = format!("{:+.0}", vol);
        draw_text(&mut frame, gx + 4, 186, &db_str, gray, cell);
        let trunc: String = name.chars().take(6).collect();
        draw_text(&mut frame, gx + 4, 170, &trunc, white, cell);
    }

    // Slots : rappel knobs 1..4 = VOL A..D (5..8 = E..H sur les potards).
    for (i, gi) in [0usize, 1, 2, 3].iter().enumerate() {
        let vol = snap.project.groups.get(*gi).map(|g| g.volume_db).unwrap_or(0.0);
        let norm = ((vol + 60.0) / 72.0).clamp(0.0, 1.0);
        draw_encoder_slot(
            &mut frame,
            8 + i * 118,
            208,
            112,
            60,
            &format!("{}:VOL {}", i + 1, (b'A' + *gi as u8) as char),
            &format!("{:+.1}dB", vol),
            norm,
            cyan,
        );
    }
    frame
}

/// Écran gauche — vue BROWSER : liste des 16 sons du groupe focus + sélection.
/// Knob 1 = son, knob 2 = groupe.
pub fn render_browser_left(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Left, bg);
    let cyan = DisplayPixel::from_rgb(0, 200, 255);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let green = DisplayPixel::from_rgb(0, 220, 100);

    let tabs = ["1:ARRANGE", "2:MIXER", "3:SAMPLER", "4:BROWSER"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        draw_button_tab(&mut frame, tx, 2, 114, 22, tab, i == 3, cyan);
    }
    let group_char = (b'A' + snap.group_focus) as char;
    draw_text(
        &mut frame,
        8,
        28,
        &format!("BROWSER GRP {}  |  PAD {:02}", group_char, snap.active_sound + 1),
        yellow,
        bg,
    );

    // Colonne 1 : Cible (Pads 1..16 compact, 8 lignes x 2 colonnes dans la moitié gauche)
    draw_text(&mut frame, 8, 44, "CIBLE (Taper Pad / Knob 1) :", gray, bg);
    if let Some(group) = snap.project.groups.get(snap.group_focus as usize) {
        for i in 0..16usize {
            let col = i / 8;
            let row = i % 8;
            let px = 8 + col * 114;
            let py = 58 + row * 18;
            let is_sel = i as u8 == snap.active_sound;
            let cell = if is_sel {
                DisplayPixel::from_rgb(40, 60, 50)
            } else {
                DisplayPixel::from_rgb(22, 25, 34)
            };
            fill_rect(&mut frame, px, py, 110, 16, cell);
            if is_sel {
                draw_rect_outline(&mut frame, px, py, 110, 16, green);
            }
            let nm = group
                .sounds
                .get(i)
                .and_then(|s| s.as_ref())
                .map(|s| s.name.as_str())
                .unwrap_or("-");
            let trunc: String = nm.chars().take(8).collect();
            let line = format!("{:02} {}", i + 1, trunc);
            draw_text(
                &mut frame,
                px + 3,
                py + 1,
                &line,
                if is_sel { green } else { white },
                cell,
            );
        }
    }

    // Colonne 2 : Presets disponibles (16 sons, 8 lignes x 2 sous-colonnes dans la moitié droite)
    draw_text(&mut frame, 244, 44, "PRESETS 808 (4D / Knob 2) :", cyan, bg);
    for i in 0..16usize {
        let col = i / 8;
        let row = i % 8;
        let px = 244 + col * 114;
        let py = 58 + row * 18;
        let is_sel = i as u8 == snap.browser_preset_idx;
        let cell = if is_sel {
            DisplayPixel::from_rgb(10, 50, 70)
        } else {
            DisplayPixel::from_rgb(20, 22, 30)
        };
        fill_rect(&mut frame, px, py, 110, 16, cell);
        if is_sel {
            draw_rect_outline(&mut frame, px, py, 110, 16, cyan);
        }
        let nm = crate::engine::DRUM_PRESET_NAMES
            .get(i)
            .copied()
            .unwrap_or("-");
        let prefix = if is_sel { ">" } else { " " };
        let trunc: String = nm.chars().take(9).collect();
        let line = format!("{}{:02} {}", prefix, i + 1, trunc);
        draw_text(
            &mut frame,
            px + 2,
            py + 1,
            &line,
            if is_sel { cyan } else { white },
            cell,
        );
    }

    let snd_norm = snap.active_sound as f32 / 15.0;
    let preset_norm = snap.browser_preset_idx as f32 / 15.0;
    let sel_preset_name = crate::engine::DRUM_PRESET_NAMES
        .get(snap.browser_preset_idx as usize)
        .copied()
        .unwrap_or("-");

    draw_encoder_slot(
        &mut frame,
        8,
        210,
        150,
        56,
        "1:PAD CIBLE",
        &format!("PAD {:02}", snap.active_sound + 1),
        snd_norm,
        green,
    );
    draw_encoder_slot(
        &mut frame,
        164,
        210,
        150,
        56,
        "2:PRESET (4D)",
        sel_preset_name,
        preset_norm,
        cyan,
    );
    draw_encoder_slot(
        &mut frame,
        320,
        210,
        150,
        56,
        "8:MASTER",
        &format!("{:.0}%", snap.master_volume * 100.0),
        snap.master_volume,
        yellow,
    );
    frame
}

/// Écran droit — vue BROWSER : validation et chargement du preset sélectionné.
pub fn render_browser_right(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Right, bg);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let green = DisplayPixel::from_rgb(0, 220, 100);
    let cyan = DisplayPixel::from_rgb(0, 200, 255);

    let tabs = ["5:PATTERN", "6:EVENTS", "7:MACRO", "8:CHARGER"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        let is_load = i == 3;
        draw_button_tab(
            &mut frame,
            tx,
            2,
            114,
            22,
            tab,
            is_load,
            if is_load { green } else { yellow },
        );
    }

    let group_char = (b'A' + snap.group_focus) as char;
    draw_text(
        &mut frame,
        8,
        28,
        "CHARGEMENT SON / INSTRUMENT",
        yellow,
        bg,
    );

    let sel_preset_name = crate::engine::DRUM_PRESET_NAMES
        .get(snap.browser_preset_idx as usize)
        .copied()
        .unwrap_or("-");

    let cur_sound_name = snap
        .project
        .groups
        .get(snap.group_focus as usize)
        .and_then(|g| g.sounds.get(snap.active_sound as usize).and_then(|s| s.as_ref()))
        .map(|s| s.name.as_str())
        .unwrap_or("-");

    // Cadre central de prévisualisation et action
    fill_rect(&mut frame, 12, 50, 456, 120, DisplayPixel::from_rgb(22, 28, 38));
    draw_rect_outline(&mut frame, 12, 50, 456, 120, green);

    draw_text(&mut frame, 24, 58, "PRESET A CHARGER :", gray, DisplayPixel::from_rgb(22, 28, 38));
    let preset_header = format!("▶  {}  ◀", sel_preset_name);
    draw_text(&mut frame, 24, 74, &preset_header, cyan, DisplayPixel::from_rgb(22, 28, 38));

    let target_info = format!(
        "CIBLE : GRP {} > PAD {:02} (actuel : {})",
        group_char,
        snap.active_sound + 1,
        cur_sound_name
    );
    draw_text(&mut frame, 24, 98, &target_info, white, DisplayPixel::from_rgb(22, 28, 38));

    let action_box_y = 122;
    fill_rect(&mut frame, 24, action_box_y, 432, 34, DisplayPixel::from_rgb(0, 60, 30));
    draw_rect_outline(&mut frame, 24, action_box_y, 432, 34, green);
    draw_text(
        &mut frame,
        36,
        action_box_y + 4,
        "APPUYEZ SUR L'ENCODEUR 4D OU LE BOUTON 8",
        green,
        DisplayPixel::from_rgb(0, 60, 30),
    );
    draw_text(
        &mut frame,
        36,
        action_box_y + 18,
        "POUR CHARGER CE SON SUR LE PAD !",
        white,
        DisplayPixel::from_rgb(0, 60, 30),
    );

    // Instructions rapides
    draw_text(&mut frame, 12, 182, "• Tourner 4D ou Knob 2 : Parcourir presets (+ pre-ecoute)", gray, bg);
    draw_text(&mut frame, 12, 200, "• Taper un Pad ou Knob 1 : Choisir le pad cible", gray, bg);
    draw_text(&mut frame, 12, 218, "• Appuyer 4D / Bouton 8 : Valider et charger le son", green, bg);
    draw_text(&mut frame, 12, 236, "• Bouton BROWSER : Quitter le browser et revenir", gray, bg);

    frame
}

/// Écran droit — vue EVENTS : liste des événements du pattern actif.
pub fn render_events_right(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Right, bg);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);

    let tabs = ["5:PATTERN", "6:EVENTS", "7:MACRO", "8:SETTINGS"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        draw_button_tab(&mut frame, tx, 2, 114, 22, tab, i == 1, yellow);
    }
    let group_char = (b'A' + snap.group_focus) as char;
    draw_text(&mut frame, 8, 28, &format!("EVENTS GRP {} PAT 1", group_char), yellow, bg);

    if let Some(pat) = snap
        .project
        .groups
        .get(snap.group_focus as usize)
        .and_then(|g| g.patterns.first())
    {
        draw_text(&mut frame, 8, 46, &format!("{} notes - {}", pat.events.len(), pat.name), gray, bg);
        let mut y = 66;
        for ev in pat.events.iter().take(9) {
            let bar = ev.tick / 3840 + 1;
            let beat = (ev.tick % 3840) / 960 + 1;
            let step = ((ev.tick % 960) / 240) + 1;
            let line = format!("P{:02} b{}:{}:s{} v{}", ev.pad + 1, bar, beat, step, ev.velocity);
            draw_text(&mut frame, 12, y, &line, white, bg);
            y += 18;
        }
        if pat.events.len() > 9 {
            draw_text(&mut frame, 12, y, &format!("... +{} notes", pat.events.len() - 9), gray, bg);
        }
    } else {
        draw_text(&mut frame, 8, 46, "Pattern vide", gray, bg);
    }

    let master_pct = format!("{:.0}%", snap.master_volume * 100.0);
    draw_encoder_slot(&mut frame, 8, 208, 112, 60, "5:QUANT", "1/16", 0.5, yellow);
    draw_encoder_slot(&mut frame, 124, 208, 112, 60, "6:SWING", &format!("{:.0}%", snap.project.swing * 100.0), snap.project.swing, yellow);
    draw_encoder_slot(&mut frame, 240, 208, 112, 60, "7:HUMAN", "VAR", 0.5, yellow);
    draw_encoder_slot(&mut frame, 356, 208, 112, 60, "8:MASTER", &master_pct, snap.master_volume, yellow);
    frame
}

/// Écran droit — vue MACRO : macros master/groupe.
pub fn render_macro_right(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Right, bg);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let green = DisplayPixel::from_rgb(0, 220, 100);

    let tabs = ["5:PATTERN", "6:EVENTS", "7:MACRO", "8:SETTINGS"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        draw_button_tab(&mut frame, tx, 2, 114, 22, tab, i == 2, yellow);
    }
    draw_text(&mut frame, 8, 28, "MACRO - MASTER / GROUP", yellow, bg);

    let gv = snap.project.groups.get(snap.group_focus as usize).map(|g| g.volume_db).unwrap_or(0.0);
    let gp = snap.project.groups.get(snap.group_focus as usize).map(|g| g.pan).unwrap_or(0.0);
    let group_char = (b'A' + snap.group_focus) as char;
    // Barres : master, grp vol, swing, bpm(norm 40..240).
    let rows = [
        ("MASTER", snap.master_volume),
        ("GRP VOL", ((gv + 60.0) / 72.0).clamp(0.0, 1.0)),
        ("SWING", snap.project.swing.clamp(0.0, 1.0)),
        ("TEMPO", ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0)),
    ];
    let mut y = 52;
    for (name, norm) in rows {
        draw_text(&mut frame, 8, y, name, white, bg);
        fill_rect(&mut frame, 110, y + 2, 350, 12, DisplayPixel::from_rgb(24, 28, 36));
        let fill = ((norm * 350.0) as usize).min(350);
        if fill > 0 {
            fill_rect(&mut frame, 110, y + 2, fill, 12, green);
        }
        y += 24;
    }
    draw_text(
        &mut frame,
        8,
        y + 4,
        &format!("GRP {} {:+.1}dB pan {:+.2}  BPM {:.1}", group_char, gv, gp, snap.transport.bpm),
        gray,
        bg,
    );

    draw_encoder_slot(&mut frame, 8, 208, 112, 60, "5:MASTER", &format!("{:.0}%", snap.master_volume * 100.0), snap.master_volume, yellow);
    draw_encoder_slot(&mut frame, 124, 208, 112, 60, "6:GRPVOL", &format!("{:+.1}dB", gv), ((gv + 60.0) / 72.0).clamp(0.0, 1.0), yellow);
    draw_encoder_slot(&mut frame, 240, 208, 112, 60, "7:SWING", &format!("{:.0}%", snap.project.swing * 100.0), snap.project.swing, yellow);
    draw_encoder_slot(&mut frame, 356, 208, 112, 60, "8:TEMPO", &format!("{:.1}", snap.transport.bpm), ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0), yellow);
    frame
}

/// Écran droit — vue SETTINGS : transport + raccourcis hardware.
pub fn render_settings_right(snap: &crate::engine::EngineSnapshot) -> DisplayFrame {
    let bg = DisplayPixel::from_rgb(14, 15, 20);
    let mut frame = DisplayFrame::solid(DisplayId::Right, bg);
    let yellow = DisplayPixel::from_rgb(255, 200, 0);
    let white = DisplayPixel::WHITE;
    let gray = DisplayPixel::from_rgb(120, 130, 150);
    let green = DisplayPixel::from_rgb(0, 220, 100);

    let tabs = ["5:PATTERN", "6:EVENTS", "7:MACRO", "8:SETTINGS"];
    for (i, &tab) in tabs.iter().enumerate() {
        let tx = 4 + i * 118;
        draw_button_tab(&mut frame, tx, 2, 114, 22, tab, i == 3, yellow);
    }
    draw_text(&mut frame, 8, 28, "SETTINGS", yellow, bg);

    let loop_txt = if snap.transport.loop_enabled { "LOOP ON" } else { "LOOP OFF" };
    let met_txt = if snap.transport.metronome { "METRO ON" } else { "METRO OFF" };
    draw_text(&mut frame, 8, 52, &format!("BPM {:.1}  SWING {:.0}%", snap.transport.bpm, snap.project.swing * 100.0), white, bg);
    draw_text(&mut frame, 8, 72, &format!("{}  {}  (Shift+RST / Shift+TAP)", loop_txt, met_txt), if snap.transport.loop_enabled { green } else { gray }, bg);
    draw_text(&mut frame, 8, 96, "SCENE/PAT/SEL/ERASE = modes pads", gray, bg);
    draw_text(&mut frame, 8, 114, "MUTE/SOLO+GROUP = mute/solo grp", gray, bg);
    draw_text(&mut frame, 8, 132, "SAMPLING = edit son (k1-8)", gray, bg);
    draw_text(&mut frame, 8, 150, "MIXER = vol A-H (k1-8)", gray, bg);
    let pad_map = std::env::var("MASCHINE_PAD_MAP").unwrap_or_else(|_| "hw".to_string());
    draw_text(&mut frame, 8, 172, &format!("PAD MAP: {} (pad-calibrate)", pad_map), gray, bg);

    draw_encoder_slot(&mut frame, 8, 208, 112, 60, "5:BPM", &format!("{:.1}", snap.transport.bpm), ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0), yellow);
    draw_encoder_slot(&mut frame, 124, 208, 112, 60, "6:SWING", &format!("{:.0}%", snap.project.swing * 100.0), snap.project.swing, yellow);
    draw_encoder_slot(&mut frame, 240, 208, 112, 60, "7:LOOP", loop_txt, if snap.transport.loop_enabled { 1.0 } else { 0.0 }, yellow);
    draw_encoder_slot(&mut frame, 356, 208, 112, 60, "8:METRO", met_txt, if snap.transport.metronome { 1.0 } else { 0.0 }, yellow);
    frame
}
