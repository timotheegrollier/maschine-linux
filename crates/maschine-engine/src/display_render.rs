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

// ═══════════════════════════════════════════════════════════════════════
// Design system écrans — style logiciel Maschine officiel
// Bandeau titre, contenu dense, rangée d'encodeurs réels, soft keys
// alignées sur les 8 boutons physiques (labels = actions réelles).
// ═══════════════════════════════════════════════════════════════════════

use crate::engine::{EngineSnapshot, DRUM_PRESET_NAMES};
use crate::project::{Adsr, Pattern};
use crate::transport::TransportState;

/// Largeur écran (px).
const SCR_W: usize = 480;
/// Hauteur écran (px).
const SCR_H: usize = 272;
/// Hauteur rangée soft keys.
const FOOTER_H: usize = 24;
/// Hauteur rangée encodeurs.
const ENC_H: usize = 48;
/// Ticks par mesure (4/4, PPQ 960).
const TICKS_PER_BAR: u64 = 3840;
/// Ticks par pas (double-croche).
const TICKS_PER_STEP: u64 = 240;

/// Palette unifiée des écrans.
struct Pal {
    bg: DisplayPixel,
    panel: DisplayPixel,
    panel2: DisplayPixel,
    border: DisplayPixel,
    dim: DisplayPixel,
    muted: DisplayPixel,
    white: DisplayPixel,
    accent: DisplayPixel,
    cyan: DisplayPixel,
    green: DisplayPixel,
    red: DisplayPixel,
    yellow: DisplayPixel,
    track: DisplayPixel,
}

impl Pal {
    fn new() -> Self {
        Self {
            bg: DisplayPixel::from_rgb(12, 13, 17),
            panel: DisplayPixel::from_rgb(24, 26, 33),
            panel2: DisplayPixel::from_rgb(34, 37, 47),
            border: DisplayPixel::from_rgb(48, 52, 64),
            dim: DisplayPixel::from_rgb(158, 166, 180),
            muted: DisplayPixel::from_rgb(96, 103, 118),
            white: DisplayPixel::WHITE,
            accent: DisplayPixel::from_rgb(255, 178, 0),
            cyan: DisplayPixel::from_rgb(0, 196, 255),
            green: DisplayPixel::from_rgb(94, 222, 128),
            red: DisplayPixel::from_rgb(255, 86, 86),
            yellow: DisplayPixel::from_rgb(235, 205, 60),
            track: DisplayPixel::from_rgb(38, 41, 52),
        }
    }

    /// Couleur de niveau VU (vert -> jaune -> rouge).
    fn vu(&self, level: f32) -> DisplayPixel {
        if level >= 0.86 {
            self.red
        } else if level >= 0.62 {
            self.yellow
        } else {
            self.green
        }
    }
}

// ── Helpers de dessin ──────────────────────────────────────────────────

/// Troncature sûre (par caractères).
fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Couleur de pad/Sound (16 teintes).
fn hue_color(index: usize) -> DisplayPixel {
    let h = (index % 16) as f32 / 16.0;
    let s = 0.62_f32;
    let v = 0.96_f32;
    let hi = (h * 6.0) as i32 % 6;
    let f = h * 6.0 - (h * 6.0).floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match hi {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    DisplayPixel::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn fmt_db(db: f32) -> String {
    if db <= -59.5 {
        "-inf".to_string()
    } else {
        format!("{db:+.1}")
    }
}

fn db_norm(db: f32) -> f32 {
    ((db + 60.0) / 72.0).clamp(0.0, 1.0)
}

fn lin_db(v: f32) -> f32 {
    if v <= 1.0e-4 {
        -60.0
    } else {
        20.0 * v.log10()
    }
}

fn fmt_pan(pan: f32) -> String {
    let p = pan.clamp(-1.0, 1.0);
    if p.abs() < 0.02 {
        "C".to_string()
    } else if p < 0.0 {
        format!("L{:.0}", -p * 100.0)
    } else {
        format!("R{:.0}", p * 100.0)
    }
}

fn fmt_ms(ms: f32) -> String {
    if ms >= 1000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else if ms >= 100.0 {
        format!("{ms:.0}ms")
    } else {
        format!("{ms:.1}ms")
    }
}

fn fmt_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1}k", hz / 1000.0)
    } else {
        format!("{hz:.0}Hz")
    }
}

fn draw_char_scaled(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    c: char,
    scale: usize,
    fg: DisplayPixel,
    bg: DisplayPixel,
) {
    let ascii = c as usize;
    let glyph = if (32..=126).contains(&ascii) {
        &FONT_8X16_BASIC[ascii - 32]
    } else {
        &FONT_8X16_BASIC[0]
    };
    let fw = frame.w as usize;
    let fh = frame.h as usize;
    for (row, &bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            let color = if (bits & (0x80 >> col)) != 0 { fg } else { bg };
            for sy in 0..scale {
                let py = y + row * scale + sy;
                if py >= fh {
                    continue;
                }
                for sx in 0..scale {
                    let px = x + col * scale + sx;
                    if px >= fw {
                        continue;
                    }
                    let idx = py * fw + px;
                    if idx < frame.pixels.len() {
                        frame.pixels[idx] = color;
                    }
                }
            }
        }
    }
}

fn draw_text_scaled(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    text: &str,
    scale: usize,
    fg: DisplayPixel,
    bg: DisplayPixel,
) {
    for (i, c) in text.chars().enumerate() {
        draw_char_scaled(frame, x + i * 8 * scale, y, c, scale, fg, bg);
    }
}

/// Tracé de segment (Bresenham).
fn line(frame: &mut DisplayFrame, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: DisplayPixel) {
    let fw = frame.w as usize;
    let fh = frame.h as usize;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as usize) < fw && (y0 as usize) < fh {
            let idx = y0 as usize * fw + x0 as usize;
            if idx < frame.pixels.len() {
                frame.pixels[idx] = color;
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Bandeau titre avec filet accent (haut des 2 écrans).
fn header(frame: &mut DisplayFrame, p: &Pal, title: &str, right: &str) {
    fill_rect(frame, 0, 0, SCR_W, 22, p.panel);
    fill_rect(frame, 0, 0, 4, 22, p.accent);
    draw_text(frame, 10, 3, title, p.white, p.panel);
    let mut txt = right.to_string();
    if txt.len() * 8 > SCR_W - 60 {
        txt = trunc(&txt, (SCR_W - 60) / 8);
    }
    let l = txt.len() * 8;
    draw_text(frame, SCR_W - l - 10, 3, &txt, p.dim, p.panel);
    fill_rect(frame, 0, 22, SCR_W, 1, p.border);
}

/// Rangée de 4 soft keys (celles de ce côté de l'écran) : labels = actions.
fn footer_softkeys(
    frame: &mut DisplayFrame,
    p: &Pal,
    labels: &[&'static str; 8],
    start: usize,
    active: &[bool; 8],
) {
    let y = SCR_H - FOOTER_H;
    for i in 0..4 {
        let idx = start + i;
        let x = i * 120;
        let on = active.get(idx).copied().unwrap_or(false);
        let bg = if on { p.accent } else { p.panel };
        let fg = if on { p.bg } else { p.dim };
        fill_rect(frame, x + 2, y, 116, FOOTER_H - 2, bg);
        draw_rect_outline(
            frame,
            x + 2,
            y,
            116,
            FOOTER_H - 2,
            if on { p.accent } else { p.border },
        );
        let label = labels[idx];
        let tx = x + 2 + (116usize.saturating_sub(label.len() * 8)) / 2;
        draw_text(frame, tx, y + 3, label, fg, bg);
    }
}

/// Barre horizontale de valeur.
#[allow(clippy::too_many_arguments)] // helpers de dessin : coordonnées explicites
fn bar_h(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    frac: f32,
    color: DisplayPixel,
) {
    fill_rect(frame, x, y, w, h, p.track);
    let f = (frac.clamp(0.0, 1.0) * w as f32) as usize;
    if f > 0 {
        fill_rect(frame, x, y, f.min(w), h, color);
    }
}

/// VU-mètre horizontal avec peak hold.
#[allow(clippy::too_many_arguments)] // helpers de dessin : coordonnées explicites
fn vu_h(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    level: f32,
    peak: f32,
) {
    fill_rect(frame, x, y, w, h, p.track);
    let f = (level.clamp(0.0, 1.0) * w as f32) as usize;
    if f > 0 {
        fill_rect(frame, x, y, f.min(w), h, p.vu(level));
    }
    if peak > 0.02 {
        let px = (x + ((peak.clamp(0.0, 1.0) * w as f32) as usize)).min(x + w - 2);
        fill_rect(frame, px, y, 2, h, p.white);
    }
}

/// VU-mètre vertical avec peak hold.
#[allow(clippy::too_many_arguments)] // helpers de dessin : coordonnées explicites
fn vu_v(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    level: f32,
    peak: f32,
) {
    fill_rect(frame, x, y, w, h, p.track);
    let f = (level.clamp(0.0, 1.0) * h as f32) as usize;
    if f > 0 {
        fill_rect(frame, x, y + h - f.min(h), w, f.min(h), p.vu(level));
    }
    if peak > 0.02 {
        let pf = ((peak.clamp(0.0, 1.0) * h as f32) as usize).clamp(1, h.saturating_sub(1));
        fill_rect(frame, x, y + h - pf, w, 2, p.white);
    }
}

/// Pastille d'état (M, S, onglet de mode...).
#[allow(clippy::too_many_arguments)] // helpers de dessin : coordonnées explicites
fn pill(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    label: &str,
    color: DisplayPixel,
    on: bool,
) {
    let bg = if on { color } else { p.panel2 };
    let fg = if on { p.bg } else { p.muted };
    fill_rect(frame, x, y, w, 16, bg);
    draw_rect_outline(frame, x, y, w, 16, if on { color } else { p.border });
    let tx = x + (w.saturating_sub(label.len() * 8)) / 2;
    draw_text(frame, tx, y, label, fg, bg);
}

/// Texte centré horizontalement.
fn center_text(frame: &mut DisplayFrame, y: usize, text: &str, fg: DisplayPixel, bg: DisplayPixel) {
    let x = SCR_W.saturating_sub(text.len() * 8) / 2;
    draw_text(frame, x, y, text, fg, bg);
}

/// Slot d'encodeur : (label, valeur, normalisation 0..1, couleur).
type EncSlot<'a> = Option<(&'a str, String, f32, DisplayPixel)>;

/// Rangée de 4 encodeurs (bas de chaque écran, au-dessus des soft keys).
fn encoder_row(frame: &mut DisplayFrame, p: &Pal, slots: &[EncSlot<'_>; 4]) {
    let y = SCR_H - FOOTER_H - ENC_H - 6;
    for (i, slot) in slots.iter().enumerate() {
        let x = 8 + i * 116;
        let (label, value, norm, color) = match slot {
            Some((l, v, n, c)) => (*l, v.as_str(), *n, *c),
            None => ("--", "", 0.0, p.border),
        };
        fill_rect(frame, x, y, 110, ENC_H, p.panel);
        draw_rect_outline(frame, x, y, 110, ENC_H, p.border);
        draw_text(frame, x + 6, y + 4, &trunc(label, 13), p.muted, p.panel);
        draw_text(frame, x + 6, y + 20, &trunc(value, 13), p.white, p.panel);
        bar_h(frame, p, x + 6, y + ENC_H - 10, 98, 5, norm, color);
    }
}

// ── Accès données (snapshot) ───────────────────────────────────────────

fn focus_group(snap: &EngineSnapshot) -> Option<&crate::project::Group> {
    snap.project.groups.get(snap.group_focus as usize)
}

fn focus_sound(snap: &EngineSnapshot) -> Option<&crate::project::Sound> {
    focus_group(snap)?
        .sounds
        .get(snap.active_sound as usize)?
        .as_ref()
}

fn sound_name(snap: &EngineSnapshot, sound: usize) -> String {
    snap.project
        .groups
        .get(snap.group_focus as usize)
        .and_then(|g| g.sounds.get(sound))
        .and_then(|s| s.as_ref())
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "--".to_string())
}

fn group_name(snap: &EngineSnapshot, group: usize) -> String {
    snap.project
        .groups
        .get(group)
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "--".to_string())
}

fn group_letter(group: u8) -> char {
    (b'A' + group.min(25)) as char
}

/// Pattern actif du groupe focus dans la scène active.
fn active_pattern(snap: &EngineSnapshot) -> Option<&Pattern> {
    let gi = snap.group_focus as usize;
    let g = snap.project.groups.get(gi)?;
    let pi = snap.project.active_pattern_index(gi, snap.active_scene)?;
    g.patterns.get(pi)
}

fn current_tick(snap: &EngineSnapshot) -> u64 {
    snap.transport.tick
}

fn tick_bar(snap: &EngineSnapshot) -> usize {
    ((current_tick(snap) / TICKS_PER_BAR) as usize) % snap.pattern_bars.max(1)
}

fn tick_step(snap: &EngineSnapshot) -> usize {
    ((current_tick(snap) / TICKS_PER_STEP) % 16) as usize
}

// ── Sous-blocs réutilisables ───────────────────────────────────────────

/// Waveform du Sound actif (enveloppe min/max par colonne).
fn draw_waveform(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    buckets: &[(f32, f32)],
) {
    fill_rect(frame, x, y, w, h, p.panel);
    draw_rect_outline(frame, x, y, w, h, p.border);
    let cy = y + h / 2;
    fill_rect(frame, x + 1, cy, w - 2, 1, p.track);
    if buckets.is_empty() {
        center_text(frame, y + h / 2 - 8, "AUCUN SAMPLE", p.muted, p.panel);
        center_text(
            frame,
            y + h / 2 + 10,
            "BROWSER POUR CHARGER",
            p.muted,
            p.panel,
        );
        return;
    }
    let n = buckets.len().max(1);
    let half = (h / 2 - 8) as f32;
    for (i, (mn, mx)) in buckets.iter().enumerate() {
        let px = x + 2 + (i * (w - 4)) / n;
        let top = cy as i32 - (mx.clamp(-1.0, 1.0) * half) as i32;
        let bot = cy as i32 - (mn.clamp(-1.0, 1.0) * half) as i32;
        let (t, b) = if top <= bot { (top, bot) } else { (bot, top) };
        let t = t.max(y as i32 + 1) as usize;
        let b = b.min((y + h) as i32 - 2) as usize;
        if b >= t {
            fill_rect(frame, px, t, 2, b - t + 1, p.green);
        }
    }
}

/// Courbe ADSR (A/D/S/R) avec vraies valeurs.
fn draw_adsr(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    adsr: &Adsr,
) {
    fill_rect(frame, x, y, w, h, p.panel);
    draw_rect_outline(frame, x, y, w, h, p.border);

    let a = (adsr.attack_ms / 500.0).clamp(0.0, 1.0);
    let d = (adsr.decay_ms / 1000.0).clamp(0.0, 1.0);
    let s = adsr.sustain.clamp(0.0, 1.0);
    let r = (adsr.release_ms / 2000.0).clamp(0.0, 1.0);
    let total = (a + d + 0.4 + r).max(0.001);
    let iw = w.saturating_sub(16);
    let ih = h.saturating_sub(20);
    let x0 = x + 8;
    let yb = y + h - 10;
    let yt = yb - ((s * ih as f32) as usize).max(2);
    let xa = x0 + ((a / total) * iw as f32) as usize;
    let xd = xa + ((d / total) * iw as f32) as usize;
    let xs = xd + ((0.4 / total) * iw as f32) as usize;
    let xr = (x0 + iw).min(xs + ((r / total) * iw as f32) as usize);

    line(
        frame,
        x0 as i32,
        yb as i32,
        xa as i32,
        y as i32 + 8,
        p.accent,
    );
    line(
        frame,
        xa as i32,
        y as i32 + 8,
        xd as i32,
        yt as i32,
        p.accent,
    );
    line(frame, xd as i32, yt as i32, xs as i32, yt as i32, p.accent);
    line(frame, xs as i32, yt as i32, xr as i32, yb as i32, p.accent);
    fill_rect(frame, x0, yb, iw, 1, p.track);
    draw_text(
        frame,
        x + 6,
        y + h - 18,
        &format!(
            "A{} D{} S{:.0}% R{}",
            fmt_ms(adsr.attack_ms),
            fmt_ms(adsr.decay_ms),
            adsr.sustain * 100.0,
            fmt_ms(adsr.release_ms)
        ),
        p.muted,
        p.panel,
    );
}

// ── ARRANGE (écran gauche) ─────────────────────────────────────────────

/// Écran gauche — vue ARRANGE : 8 groupes, pattern de la scène, VU réels.
#[must_use]
pub fn render_daw_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let playing = snap.transport.is_playing();
    let recording = snap.transport.state == TransportState::Recording;

    let scene_lbl = match snap.active_scene {
        Some(s) => format!("SCENE {}/{}", s + 1, snap.project.scenes.len().max(1)),
        None => format!("SCENE -/{}", snap.project.scenes.len().max(1)),
    };
    header(
        &mut frame,
        &p,
        "ARRANGE",
        &format!("{scene_lbl} | {:.0} BPM", snap.transport.bpm),
    );

    // Bandeau transport : état + position mes/pas + loop/métro.
    let (state_txt, state_col) = if recording {
        ("ENREG", p.red)
    } else if playing {
        ("LECTURE", p.green)
    } else {
        ("STOP", p.panel2)
    };
    pill(
        &mut frame,
        &p,
        8,
        27,
        84,
        state_txt,
        state_col,
        playing || recording,
    );
    let bar = tick_bar(snap) + 1;
    let step = tick_step(snap) + 1;
    draw_text(
        &mut frame,
        100,
        28,
        &format!("BAR {bar}  PAS {step:>2}"),
        p.white,
        p.bg,
    );
    draw_text(
        &mut frame,
        232,
        28,
        &format!("MES {} / {}", snap.selected_bar + 1, snap.pattern_bars),
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        360,
        28,
        "LOOP",
        if snap.transport.loop_enabled {
            p.cyan
        } else {
            p.track
        },
        p.bg,
    );
    draw_text(
        &mut frame,
        424,
        28,
        "METRO",
        if snap.transport.metronome {
            p.cyan
        } else {
            p.track
        },
        p.bg,
    );
    fill_rect(&mut frame, 0, 45, SCR_W, 1, p.border);

    // Liste des 8 groupes : lettre, nom, pattern, M/S, VU temps réel.
    let mut y = 50;
    for gi in 0..8usize {
        let g = snap.project.groups.get(gi);
        let focused = gi == snap.group_focus as usize;
        let row_bg = if focused { p.panel2 } else { p.bg };
        if focused {
            fill_rect(&mut frame, 0, y - 2, SCR_W, 17, row_bg);
        }
        let letter = (b'A' + gi as u8) as char;
        let gcolor = hue_color(gi);
        let (lbg, lfg) = if focused {
            (gcolor, p.bg)
        } else {
            (p.panel, p.dim)
        };
        fill_rect(&mut frame, 6, y - 1, 13, 14, lbg);
        draw_char(&mut frame, 9, y - 2, letter, lfg, lbg);
        let name = g
            .map(|g| trunc(&g.name, 13))
            .unwrap_or_else(|| "--".to_string());
        draw_text(
            &mut frame,
            24,
            y - 2,
            &name,
            if g.is_some() { p.white } else { p.muted },
            row_bg,
        );

        let pi = snap.project.active_pattern_index(gi, snap.active_scene);
        let pat_txt = pi
            .and_then(|i| g.and_then(|g| g.patterns.get(i)))
            .map(|pat| {
                format!(
                    "{} ({}m)",
                    trunc(&pat.name, 10),
                    (pat.length.bars as usize).max(1)
                )
            })
            .unwrap_or_else(|| "--".to_string());
        draw_text(&mut frame, 148, y - 2, &pat_txt, p.dim, row_bg);

        let muted = snap.muted_groups & (1 << gi) != 0;
        let soloed = snap.soloed_groups & (1 << gi) != 0;
        draw_text(
            &mut frame,
            272,
            y - 2,
            "M",
            if muted { p.red } else { p.track },
            row_bg,
        );
        draw_text(
            &mut frame,
            286,
            y - 2,
            "S",
            if soloed { p.yellow } else { p.track },
            row_bg,
        );

        vu_h(
            &mut frame,
            &p,
            306,
            y,
            126,
            8,
            snap.group_levels[gi],
            snap.group_peaks[gi],
        );
        let vol = g.map(|g| g.volume_db).unwrap_or(0.0);
        draw_text(
            &mut frame,
            438,
            y - 2,
            &format!("{vol:>4.0}"),
            p.muted,
            row_bg,
        );
        y += 17;
    }

    // Encodeurs réels 1..4 : TEMPO, SWING, VOL GRP, PAN GRP.
    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let slots = [
        Some((
            "1:TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "2:SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.accent,
        )),
        Some(("3:VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan)),
        Some(("4:PAN GRP", fmt_pan(gpan), (gpan + 1.0) * 0.5, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    for (i, a) in active.iter_mut().take(4).enumerate() {
        *a = i == snap.selected_bar.min(3);
    }
    for (i, v) in [1usize, 2, 4, 8].iter().enumerate() {
        active[4 + i] = *v == snap.pattern_bars;
    }
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 0, &active);
    frame
}

// ── PATTERN (écran droit) ──────────────────────────────────────────────

/// Écran droit — vue PATTERN : grille 16 Sons x 16 pas de la mesure choisie.
#[must_use]
pub fn render_daw_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let playing = snap.transport.is_playing();
    let pat = active_pattern(snap);
    let (pat_name, n_events) = match pat {
        Some(pt) => (trunc(&pt.name, 14), pt.events.len()),
        None => ("--".to_string(), 0),
    };
    header(
        &mut frame,
        &p,
        "PATTERN",
        &format!(
            "GRP {} | {} | {} ev",
            group_letter(snap.group_focus),
            pat_name,
            n_events
        ),
    );

    // Masque d'événements de la mesure sélectionnée (16 sons x 16 pas).
    let bar_offset = snap.selected_bar as u32 * TICKS_PER_BAR as u32;
    let mut mask = [[false; 16]; 16];
    if let Some(pt) = pat {
        for e in &pt.events {
            if e.tick >= bar_offset && e.tick < bar_offset + TICKS_PER_BAR as u32 {
                let st = ((e.tick - bar_offset) / TICKS_PER_STEP as u32) as usize;
                let pad = e.pad as usize;
                if st < 16 && pad < 16 {
                    mask[pad][st] = true;
                }
            }
        }
    }

    // Règle des 16 pas + repère de mesure sélectionnée.
    let gx = 88usize;
    let cw = 24usize;
    for s in 0..16usize {
        let x = gx + s * cw;
        let quad = s % 4 == 0;
        draw_text(
            &mut frame,
            x + 8,
            28,
            &format!("{}", s + 1),
            if quad { p.dim } else { p.muted },
            p.bg,
        );
    }
    fill_rect(&mut frame, 0, 45, SCR_W, 1, p.border);

    let cur_bar = tick_bar(snap);
    let cur_step = tick_step(snap);
    let show_playhead = playing && cur_bar == snap.selected_bar;

    let mut y = 48;
    for (snd, row) in mask.iter().enumerate() {
        let is_active = snd == snap.active_sound as usize;
        let row_bg = if is_active { p.panel2 } else { p.bg };
        if is_active {
            fill_rect(&mut frame, 0, y - 1, SCR_W, 10, row_bg);
        }
        let label = format!("{:02} {}", snd + 1, trunc(&sound_name(snap, snd), 8));
        draw_text(
            &mut frame,
            4,
            y - 3,
            &label,
            if is_active { p.white } else { p.muted },
            row_bg,
        );
        for (st, on) in row.iter().enumerate() {
            let x = gx + st * cw;
            let col = if *on { hue_color(snd) } else { p.track };
            fill_rect(&mut frame, x + 2, y, cw - 4, 8, col);
            if *on && is_active {
                draw_rect_outline(&mut frame, x + 2, y, cw - 4, 8, p.white);
            }
        }
        y += 9;
    }

    if show_playhead {
        let x = gx + cur_step * cw;
        draw_rect_outline(&mut frame, x + 1, 45, cw - 2, 148, p.white);
        fill_rect(&mut frame, x + 2, 26, cw - 4, 3, p.white);
    }

    // Encodeurs réels 5..8 : VOL SND, CUTOFF, RESO, MASTER.
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    for (i, a) in active.iter_mut().take(4).enumerate() {
        *a = i == snap.selected_bar.min(3);
    }
    for (i, v) in [1usize, 2, 4, 8].iter().enumerate() {
        active[4 + i] = *v == snap.pattern_bars;
    }
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

// ── STEP (écran droit, mode STEP) ──────────────────────────────────────

/// Écran droit — vue STEP : pas du Sound actif (vélocités) de la mesure.
#[must_use]
pub fn render_step_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let playing = snap.transport.is_playing();
    let name = sound_name(snap, snap.active_sound as usize);
    header(
        &mut frame,
        &p,
        "STEP",
        &format!(
            "GRP {} | SND {:02} {} | MES {}/{}",
            group_letter(snap.group_focus),
            snap.active_sound + 1,
            trunc(&name, 10),
            snap.selected_bar + 1,
            snap.pattern_bars
        ),
    );

    let pat = active_pattern(snap);
    let bar_offset = snap.selected_bar as u32 * TICKS_PER_BAR as u32;
    let cur_bar = tick_bar(snap);
    let cur_step = tick_step(snap);
    let show_playhead = playing && cur_bar == snap.selected_bar;
    let pad_hue = hue_color(snap.active_sound as usize);

    let x0 = 16usize;
    let cw = 28usize;
    let top = 52usize;
    let bot = 168usize;
    for st in 0..16usize {
        let x = x0 + st * cw;
        let vel = pat
            .and_then(|pt| {
                pt.events.iter().find(|e| {
                    e.pad == snap.active_sound
                        && e.tick == bar_offset + st as u32 * TICKS_PER_STEP as u32
                })
            })
            .map(|e| e.velocity)
            .unwrap_or(0);
        fill_rect(&mut frame, x, top, cw - 2, bot - top, p.panel);
        if vel > 0 {
            let fh = (((bot - top) as f32 * (vel as f32 / 127.0)) as usize).max(3);
            fill_rect(&mut frame, x + 1, bot - fh, cw - 4, fh, pad_hue);
            draw_text(
                &mut frame,
                x + 6,
                bot - fh - 14,
                &format!("{vel}"),
                p.white,
                p.bg,
            );
        }
        draw_rect_outline(
            &mut frame,
            x,
            top,
            cw - 2,
            bot - top,
            if st % 4 == 0 { p.border } else { p.track },
        );
        draw_text(
            &mut frame,
            x + 8,
            174,
            &format!("{}", st + 1),
            if st % 4 == 0 { p.dim } else { p.muted },
            p.bg,
        );
        if show_playhead && st == cur_step {
            draw_rect_outline(&mut frame, x - 1, top - 1, cw, bot - top + 2, p.white);
        }
    }
    center_text(
        &mut frame,
        192,
        "PADS = SON | SOFT 1-4 = MESURES 5-8 = LONGUEUR",
        p.muted,
        p.bg,
    );

    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    for (i, a) in active.iter_mut().take(4).enumerate() {
        *a = i == snap.selected_bar.min(3);
    }
    for (i, v) in [1usize, 2, 4, 8].iter().enumerate() {
        active[4 + i] = *v == snap.pattern_bars;
    }
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

// ── SONG / ARRANGER ────────────────────────────────────────────────────

/// Écran gauche — vue SONG : sections de l'arrangement.
#[must_use]
pub fn render_arranger_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let active_sec = snap.active_section.unwrap_or(0);
    header(
        &mut frame,
        &p,
        "SONG",
        &format!(
            "{}/{} sections | {:.0} BPM",
            (active_sec + 1).min(snap.project.song.len().max(1)),
            snap.project.song.len(),
            snap.transport.bpm
        ),
    );

    if snap.project.song.is_empty() {
        center_text(&mut frame, 70, "AUCUNE SECTION", p.muted, p.bg);
        center_text(&mut frame, 94, "SOFT 3 = +SECTION", p.dim, p.bg);
    }

    let mut y = 30usize;
    for (si, sec) in snap.project.song.iter().enumerate().take(8) {
        let focused = si == active_sec;
        let row_bg = if focused { p.panel2 } else { p.bg };
        if focused {
            fill_rect(&mut frame, 0, y - 2, SCR_W, 19, row_bg);
            fill_rect(&mut frame, 0, y - 2, 3, 19, p.accent);
        }
        let scene_name = snap
            .project
            .scenes
            .get(sec.scene)
            .map(|s| trunc(&s.name, 12))
            .unwrap_or_else(|| "--".to_string());
        draw_text(
            &mut frame,
            10,
            y,
            &format!("{:02} {}", si + 1, trunc(&sec.name, 14)),
            if focused { p.white } else { p.dim },
            row_bg,
        );
        draw_text(
            &mut frame,
            160,
            y,
            &format!("SCENE {} {}", sec.scene + 1, scene_name),
            p.dim,
            row_bg,
        );
        draw_text(
            &mut frame,
            320,
            y,
            &format!("{} mes.", (sec.length.bars as usize).max(1)),
            p.dim,
            row_bg,
        );
        if focused && snap.transport.is_playing() {
            draw_text(&mut frame, 424, y, "LECT", p.green, row_bg);
        }
        y += 19;
    }

    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let slots = [
        Some((
            "1:TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "2:SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.accent,
        )),
        Some(("3:VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan)),
        Some(("4:PAN GRP", fmt_pan(gpan), (gpan + 1.0) * 0.5, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    let active = [false; 8];
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 0, &active);
    frame
}

/// Écran droit — vue SONG : timeline des sections + contenu de la section.
#[must_use]
pub fn render_arranger_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let total_bars: u64 = snap
        .project
        .song
        .iter()
        .map(|s| (s.length.bars as u64).max(1))
        .sum();
    header(
        &mut frame,
        &p,
        "SONG TIMELINE",
        &format!("{} mesures | BAR {}", total_bars.max(1), tick_bar(snap) + 1),
    );

    // Blocs de sections proportionnels + playhead.
    let x0 = 8usize;
    let tw = 464usize;
    let mut x = x0;
    let mut acc = 0u64;
    let tick = current_tick(snap);
    let mut playhead_x: Option<usize> = None;
    let active_sec = snap.active_section.unwrap_or(0);
    for (si, sec) in snap.project.song.iter().enumerate() {
        let bars = (sec.length.bars as u64).max(1);
        let bw = ((bars * tw as u64) / total_bars.max(1)).max(2) as usize;
        let focused = si == active_sec;
        let bg = if focused { p.panel2 } else { p.panel };
        fill_rect(&mut frame, x, 30, bw.saturating_sub(2), 40, bg);
        draw_rect_outline(
            &mut frame,
            x,
            30,
            bw.saturating_sub(2),
            40,
            if focused { p.accent } else { p.border },
        );
        if bw >= 24 {
            draw_text(
                &mut frame,
                x + 4,
                34,
                &trunc(&sec.name, (bw - 10) / 8),
                if focused { p.white } else { p.dim },
                bg,
            );
            draw_text(
                &mut frame,
                x + 4,
                52,
                &format!("S{}", sec.scene + 1),
                p.muted,
                bg,
            );
        }
        if tick >= acc * TICKS_PER_BAR && tick < (acc + bars) * TICKS_PER_BAR {
            let inner =
                (tick - acc * TICKS_PER_BAR) * bw.saturating_sub(2) as u64 / (bars * TICKS_PER_BAR);
            playhead_x = Some(x + inner as usize);
        }
        x += bw;
        acc += bars;
    }
    if let Some(px) = playhead_x {
        fill_rect(&mut frame, px, 26, 2, 48, p.white);
    }
    fill_rect(&mut frame, 0, 74, SCR_W, 1, p.border);

    // Contenu de la section active : scène + patterns par groupe.
    if let Some(sec) = snap.project.song.get(active_sec) {
        let scene = snap.project.scenes.get(sec.scene);
        draw_text(
            &mut frame,
            8,
            80,
            &format!(
                "SECTION {} {} | SCENE {} {} | {} mes.",
                active_sec + 1,
                trunc(&sec.name, 10),
                sec.scene + 1,
                scene
                    .map(|s| trunc(&s.name, 10))
                    .unwrap_or_else(|| "--".into()),
                (sec.length.bars as usize).max(1)
            ),
            p.white,
            p.bg,
        );
        let mut y = 100usize;
        for gi in 0..8usize {
            let focus = gi == snap.group_focus as usize;
            let row_bg = if focus { p.panel2 } else { p.bg };
            if focus {
                fill_rect(&mut frame, 0, y - 2, SCR_W, 12, row_bg);
            }
            draw_text(
                &mut frame,
                8,
                y - 2,
                &format!(
                    "{}  {}",
                    group_letter(gi as u8),
                    trunc(&group_name(snap, gi), 10)
                ),
                if focus { p.white } else { p.dim },
                row_bg,
            );
            let pat_txt = scene
                .and_then(|s| s.patterns.get(gi).copied().flatten())
                .and_then(|pi| snap.project.groups.get(gi)?.patterns.get(pi))
                .map(|pat| trunc(&pat.name, 12))
                .unwrap_or_else(|| "--".to_string());
            draw_text(&mut frame, 180, y - 2, &pat_txt, p.dim, row_bg);
            y += 12;
        }
    } else {
        center_text(&mut frame, 100, "AUCUNE SECTION", p.muted, p.bg);
    }

    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    active[7] = snap.active_scene == Some(0);
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

// ── SCENES / IDEAS ─────────────────────────────────────────────────────

/// Écran gauche — vue SCENES (IDEAS) : scènes + pads = scènes.
#[must_use]
pub fn render_scenes_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    header(
        &mut frame,
        &p,
        "IDEAS | SCENES",
        &format!("{} scenes | PADS = SCENES", snap.project.scenes.len()),
    );

    let mut y = 30usize;
    for (i, scene) in snap.project.scenes.iter().enumerate().take(16) {
        let col = i / 8;
        let row = i % 8;
        let x = 8 + col * 236;
        let cy = y + row * 19;
        let focused = snap.active_scene == Some(i);
        let bg = if focused { p.panel2 } else { p.panel };
        fill_rect(&mut frame, x, cy, 226, 17, bg);
        draw_rect_outline(
            &mut frame,
            x,
            cy,
            226,
            17,
            if focused { p.accent } else { p.border },
        );
        if focused {
            fill_rect(&mut frame, x, cy, 3, 17, p.accent);
        }
        draw_text(
            &mut frame,
            x + 8,
            cy + 1,
            &format!("{:02} {}", i + 1, trunc(&scene.name, 13)),
            if focused { p.white } else { p.dim },
            bg,
        );
        let count = scene.patterns.iter().filter(|p| p.is_some()).count();
        let secs = snap.project.song.iter().filter(|s| s.scene == i).count();
        draw_text(
            &mut frame,
            x + 150,
            cy + 1,
            &format!("{count} pat"),
            p.muted,
            bg,
        );
        if secs > 0 {
            draw_text(&mut frame, x + 196, cy + 1, &format!("{secs}s"), p.cyan, bg);
        }
    }
    y += 8 * 19;
    draw_text(
        &mut frame,
        8,
        y + 2,
        "SOFT 1-8 : SCENES | PADS : SELECTION DIRECTE",
        p.muted,
        p.bg,
    );
    let _ = y;

    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let slots = [
        Some((
            "1:TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "2:SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.accent,
        )),
        Some(("3:VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan)),
        Some(("4:PAN GRP", fmt_pan(gpan), (gpan + 1.0) * 0.5, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    for (i, a) in active.iter_mut().enumerate() {
        *a = snap.active_scene == Some(i);
    }
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 0, &active);
    frame
}

// ── MIXER ──────────────────────────────────────────────────────────────

/// Écran gauche — vue MIXER : groupes A-D (VU, dB, pan, M/S).
#[must_use]
pub fn render_mixer_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    header(
        &mut frame,
        &p,
        "MIXER",
        &format!("GRP A-D | MASTER {:.0}%", snap.master_volume * 100.0),
    );

    for gi in 0..4usize {
        let col = gi % 2;
        let row = gi / 2;
        let x = 8 + col * 236;
        let y = 28 + row * 84;
        let focused = gi == snap.group_focus as usize;
        let bg = if focused { p.panel2 } else { p.panel };
        fill_rect(&mut frame, x, y, 226, 78, bg);
        draw_rect_outline(
            &mut frame,
            x,
            y,
            226,
            78,
            if focused { p.accent } else { p.border },
        );
        let gcolor = hue_color(gi);
        let (lbg, lfg) = if focused {
            (gcolor, p.bg)
        } else {
            (p.track, p.dim)
        };
        fill_rect(&mut frame, x + 6, y + 6, 14, 16, lbg);
        draw_char(&mut frame, x + 9, y + 5, group_letter(gi as u8), lfg, lbg);
        draw_text(
            &mut frame,
            x + 26,
            y + 6,
            &trunc(&group_name(snap, gi), 10),
            p.white,
            bg,
        );
        let vol = snap
            .project
            .groups
            .get(gi)
            .map(|g| g.volume_db)
            .unwrap_or(0.0);
        let pan = snap.project.groups.get(gi).map(|g| g.pan).unwrap_or(0.0);
        let db_txt = fmt_db(vol);
        draw_text(
            &mut frame,
            x + 226 - 8 - db_txt.len() * 8,
            y + 6,
            &db_txt,
            p.white,
            bg,
        );
        let muted = snap.muted_groups & (1 << gi) != 0;
        let soloed = snap.soloed_groups & (1 << gi) != 0;
        draw_text(
            &mut frame,
            x + 26,
            y + 26,
            &format!("PAN {}", fmt_pan(pan)),
            p.muted,
            bg,
        );
        pill(&mut frame, &p, x + 150, y + 24, 30, "M", p.red, muted);
        pill(&mut frame, &p, x + 186, y + 24, 30, "S", p.yellow, soloed);
        // Grand VU horizontal.
        vu_h(
            &mut frame,
            &p,
            x + 6,
            y + 50,
            214,
            12,
            snap.group_levels[gi],
            snap.group_peaks[gi],
        );
        let _ = col;
    }

    let gvols = [
        ("1:VOL A", 0usize),
        ("2:VOL B", 1),
        ("3:VOL C", 2),
        ("4:VOL D", 3),
    ];
    let mut slots: [EncSlot<'_>; 4] = [None, None, None, None];
    for (i, (label, gi)) in gvols.iter().enumerate() {
        let v = snap
            .project
            .groups
            .get(*gi)
            .map(|g| g.volume_db)
            .unwrap_or(0.0);
        slots[i] = Some((label, fmt_db(v), db_norm(v), p.green));
    }
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    for (i, a) in active.iter_mut().enumerate() {
        *a = i == snap.group_focus as usize;
    }
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 0, &active);
    frame
}

/// Écran droit — vue MIXER : master + groupe focus + sound focus.
#[must_use]
pub fn render_mixer_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    header(
        &mut frame,
        &p,
        "MIXER | MASTER",
        &format!("GRP {} focus", group_letter(snap.group_focus)),
    );

    // Master : grand VU vertical + valeur.
    draw_text(&mut frame, 8, 28, "MASTER", p.dim, p.bg);
    vu_v(
        &mut frame,
        &p,
        16,
        40,
        26,
        122,
        snap.master_level,
        snap.master_peak,
    );
    let mdb = lin_db(snap.master_volume);
    draw_text_scaled(
        &mut frame,
        56,
        44,
        &fmt_db(mdb),
        2,
        if mdb > -0.5 { p.red } else { p.white },
        p.bg,
    );
    bar_h(
        &mut frame,
        &p,
        56,
        92,
        160,
        10,
        snap.master_volume,
        p.accent,
    );
    draw_text(
        &mut frame,
        56,
        106,
        &format!("VOL {:>3.0}%", snap.master_volume * 100.0),
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        56,
        126,
        &format!("CASQUE {:>3.0}%", snap.headphone_volume * 100.0),
        p.muted,
        p.bg,
    );
    let loop_txt = if snap.transport.loop_enabled {
        "LOOP ON"
    } else {
        "LOOP OFF"
    };
    draw_text(&mut frame, 56, 146, loop_txt, p.muted, p.bg);

    // Groupe focus.
    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let gi = snap.group_focus as usize;
    let muted = snap.muted_groups & (1 << gi) != 0;
    let soloed = snap.soloed_groups & (1 << gi) != 0;
    draw_text(
        &mut frame,
        250,
        28,
        &format!(
            "GROUPE {} {}",
            group_letter(snap.group_focus),
            trunc(&group_name(snap, gi), 8)
        ),
        p.white,
        p.bg,
    );
    vu_v(
        &mut frame,
        &p,
        250,
        42,
        18,
        62,
        snap.group_levels[gi.min(7)],
        snap.group_peaks[gi.min(7)],
    );
    draw_text(&mut frame, 276, 42, &fmt_db(gvol), p.white, p.bg);
    draw_text(
        &mut frame,
        276,
        60,
        &format!("PAN {}", fmt_pan(gpan)),
        p.dim,
        p.bg,
    );
    pill(&mut frame, &p, 276, 78, 28, "M", p.red, muted);
    pill(&mut frame, &p, 310, 78, 28, "S", p.yellow, soloed);
    let pat_txt = active_pattern(snap)
        .map(|pt| format!("PAT {}", trunc(&pt.name, 9)))
        .unwrap_or_else(|| "PAT --".to_string());
    draw_text(&mut frame, 276, 98, &pat_txt, p.muted, p.bg);

    // Sound focus.
    let s_name = sound_name(snap, snap.active_sound as usize);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let s_pan = focus_sound(snap).map(|s| s.pan).unwrap_or(0.0);
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    draw_text(
        &mut frame,
        250,
        116,
        &format!("SON {:02} {}", snap.active_sound + 1, trunc(&s_name, 8)),
        p.white,
        p.bg,
    );
    draw_text(
        &mut frame,
        276,
        134,
        &format!("{}  PAN {}", fmt_db(s_db), fmt_pan(s_pan)),
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        250,
        152,
        &format!("CUT {}  RESO {:.0}%", fmt_hz(cut), reso * 100.0),
        p.muted,
        p.bg,
    );

    let mut slots: [EncSlot<'_>; 4] = [None, None, None, None];
    for (i, gi) in (4..8usize).enumerate() {
        let v = snap
            .project
            .groups
            .get(gi)
            .map(|g| g.volume_db)
            .unwrap_or(0.0);
        let label = match gi {
            4 => "5:VOL E",
            5 => "6:VOL F",
            6 => "7:VOL G",
            _ => "8:VOL H",
        };
        slots[i] = Some((label, fmt_db(v), db_norm(v), p.green));
    }
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    for (i, a) in active.iter_mut().enumerate() {
        *a = i == snap.group_focus as usize;
    }
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

// ── SAMPLER ────────────────────────────────────────────────────────────

/// Écran gauche — vue SAMPLER : waveform + ADSR + infos du Sound actif.
#[must_use]
pub fn render_sampler_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let s_name = sound_name(snap, snap.active_sound as usize);
    let dur = snap.sample_duration;
    header(
        &mut frame,
        &p,
        "SAMPLER",
        &format!(
            "GRP {} | SND {:02} {} | {:.2}s",
            group_letter(snap.group_focus),
            snap.active_sound + 1,
            trunc(&s_name, 10),
            dur
        ),
    );

    let buckets = snap.waveform.as_deref().unwrap_or(&[]);
    draw_waveform(&mut frame, &p, 8, 26, 464, 92, buckets);
    draw_text(
        &mut frame,
        12,
        104,
        &format!(
            "SAMPLE {:.2}s | {} pts",
            dur,
            buckets.len().max(if buckets.is_empty() { 0 } else { 1 })
        ),
        p.dim,
        p.bg,
    );

    let adsr = focus_sound(snap)
        .map(|s| s.adsr.clone())
        .unwrap_or_default();
    draw_adsr(&mut frame, &p, 8, 124, 226, 62, &adsr);

    let snd = focus_sound(snap);
    let zones = snd.map(|s| s.zones.len()).unwrap_or(0);
    let path = snd
        .and_then(|s| s.zones.first())
        .and_then(|z| z.sample_path.as_deref())
        .map(|p| {
            p.rsplit('/')
                .next()
                .map(|f| trunc(f, 16))
                .unwrap_or_else(|| "--".into())
        })
        .unwrap_or_else(|| "--".to_string());
    let poly = snd.map(|s| s.polyphony).unwrap_or(0);
    let choke = snd
        .and_then(|s| s.choke_group)
        .map(|c| format!("{c}"))
        .unwrap_or_else(|| "--".into());
    let cut = snd.map(|s| s.filter.cutoff_hz).unwrap_or(20000.0);
    let reso = snd.map(|s| s.filter.resonance).unwrap_or(0.0);
    fill_rect(&mut frame, 242, 124, 230, 62, p.panel);
    draw_rect_outline(&mut frame, 242, 124, 230, 62, p.border);
    draw_text(
        &mut frame,
        248,
        128,
        &format!("ZONES {zones} | POLY {poly} | CHOKE {choke}"),
        p.dim,
        p.panel,
    );
    draw_text(
        &mut frame,
        248,
        146,
        &format!("FICH. {path}"),
        p.muted,
        p.panel,
    );
    draw_text(
        &mut frame,
        248,
        164,
        &format!("FILTRE {} | RESO {:.0}%", fmt_hz(cut), reso * 100.0),
        p.muted,
        p.panel,
    );

    let s_db = snd.map(|s| s.volume_db).unwrap_or(0.0);
    let s_pan = snd.map(|s| s.pan).unwrap_or(0.0);
    let slots = [
        Some(("1:VOL", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some(("2:PAN", fmt_pan(s_pan), (s_pan + 1.0) * 0.5, p.cyan)),
        Some((
            "3:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("4:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    for (i, a) in active.iter_mut().enumerate() {
        *a = i == snap.active_sound as usize;
    }
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 0, &active);
    frame
}

/// Écran droit — vue SAMPLER : ADSR plein écran + réglages du Sound.
#[must_use]
pub fn render_sampler_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let s_name = sound_name(snap, snap.active_sound as usize);
    header(
        &mut frame,
        &p,
        "SAMPLER | ENVELOPPE",
        &format!("SND {:02} {}", snap.active_sound + 1, trunc(&s_name, 12)),
    );

    let snd = focus_sound(snap);
    let adsr = snd.map(|s| s.adsr.clone()).unwrap_or_default();
    draw_adsr(&mut frame, &p, 8, 26, 464, 118, &adsr);
    // Repères A / D / S / R.
    let labels = ["A", "D", "S", "R"];
    for (i, l) in labels.iter().enumerate() {
        draw_text(&mut frame, 24 + i * 112, 146, l, p.accent, p.bg);
    }

    let s_db = snd.map(|s| s.volume_db).unwrap_or(0.0);
    let s_pan = snd.map(|s| s.pan).unwrap_or(0.0);
    let cut = snd.map(|s| s.filter.cutoff_hz).unwrap_or(20000.0);
    let reso = snd.map(|s| s.filter.resonance).unwrap_or(0.0);
    let muted = snd.map(|s| s.muted).unwrap_or(false);
    let soloed = snd.map(|s| s.solo).unwrap_or(false);
    draw_text(
        &mut frame,
        8,
        152,
        &format!("VOL {}", fmt_db(s_db)),
        p.white,
        p.bg,
    );
    draw_text(
        &mut frame,
        120,
        152,
        &format!("PAN {}", fmt_pan(s_pan)),
        p.white,
        p.bg,
    );
    draw_text(
        &mut frame,
        220,
        152,
        &format!("CUT {}", fmt_hz(cut)),
        p.white,
        p.bg,
    );
    draw_text(
        &mut frame,
        340,
        152,
        &format!("RESO {:.0}%", reso * 100.0),
        p.white,
        p.bg,
    );
    pill(&mut frame, &p, 8, 170, 60, "MUTE", p.red, muted);
    pill(&mut frame, &p, 76, 170, 60, "SOLO", p.yellow, soloed);

    let slots = [
        Some((
            "5:ATTACK",
            fmt_ms(adsr.attack_ms),
            (adsr.attack_ms / 500.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "6:DECAY",
            fmt_ms(adsr.decay_ms),
            (adsr.decay_ms / 1000.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "7:SUSTAIN",
            format!("{:.0}%", adsr.sustain * 100.0),
            adsr.sustain.clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "8:RELEASE",
            fmt_ms(adsr.release_ms),
            (adsr.release_ms / 2000.0).clamp(0.0, 1.0),
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    let mut active = [false; 8];
    for (i, a) in active.iter_mut().enumerate() {
        *a = i == snap.active_sound as usize;
    }
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

// ── BROWSER ────────────────────────────────────────────────────────────

/// Écran gauche — vue BROWSER : presets du kit 808.
#[must_use]
pub fn render_browser_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let idx = snap.browser_preset_idx as usize % DRUM_PRESET_NAMES.len();
    header(
        &mut frame,
        &p,
        "BROWSER | KIT 808",
        &format!(
            "PRESET {}/16 | MASTER {:.0}%",
            idx + 1,
            snap.master_volume * 100.0
        ),
    );

    let mut y = 28usize;
    for (i, name) in DRUM_PRESET_NAMES.iter().enumerate().take(16) {
        let col = i / 8;
        let row = i % 8;
        let x = 8 + col * 236;
        let cy = y + row * 20;
        let selected = i == idx;
        let bg = if selected { p.accent } else { p.panel };
        fill_rect(&mut frame, x, cy, 226, 18, bg);
        draw_rect_outline(
            &mut frame,
            x,
            cy,
            226,
            18,
            if selected { p.accent } else { p.border },
        );
        draw_text(
            &mut frame,
            x + 8,
            cy + 1,
            &format!("{:02} {}", i + 1, trunc(name, 13)),
            if selected { p.bg } else { p.dim },
            bg,
        );
    }
    y += 8 * 20;
    let cur = DRUM_PRESET_NAMES[idx];
    draw_text(
        &mut frame,
        8,
        y + 4,
        &format!("> {}", trunc(cur, 20)),
        p.white,
        p.bg,
    );
    draw_text(
        &mut frame,
        200,
        y + 4,
        "5 ECOUTE | 6 CHARGER",
        p.muted,
        p.bg,
    );

    let cible = sound_name(snap, snap.active_sound as usize);
    let slots = [
        Some((
            "1:SND CIBLE",
            format!("{:02} {}", snap.active_sound + 1, trunc(&cible, 8)),
            snap.active_sound as f32 / 15.0,
            p.cyan,
        )),
        Some(("2:PRESET", trunc(cur, 13), idx as f32 / 15.0, p.accent)),
        Some(("3:--", String::new(), 0.0, p.border)),
        Some(("4:--", String::new(), 0.0, p.border)),
    ];
    encoder_row(&mut frame, &p, &slots);

    let active = [false; 8];
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 0, &active);
    frame
}

/// Écran droit — vue BROWSER : preset sélectionné + cible de chargement.
#[must_use]
pub fn render_browser_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let idx = snap.browser_preset_idx as usize % DRUM_PRESET_NAMES.len();
    let name = DRUM_PRESET_NAMES[idx];
    header(
        &mut frame,
        &p,
        "BROWSER | CHARGER",
        &format!("PRESET {}/16", idx + 1),
    );

    draw_text_scaled(&mut frame, 12, 40, &trunc(name, 26), 2, p.white, p.bg);
    draw_text(&mut frame, 12, 84, "-> CIBLE", p.muted, p.bg);
    draw_text(
        &mut frame,
        12,
        102,
        &format!(
            "GROUPE {} | SND {:02} {}",
            group_letter(snap.group_focus),
            snap.active_sound + 1,
            trunc(&sound_name(snap, snap.active_sound as usize), 14)
        ),
        p.cyan,
        p.bg,
    );
    draw_text(
        &mut frame,
        12,
        128,
        "k1 SND CIBLE | k2 PRESET | k8 MASTER",
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        12,
        146,
        "SOFT 5 ECOUTE | SOFT 6 CHARGER",
        p.dim,
        p.bg,
    );
    pill(&mut frame, &p, 12, 168, 90, "ECOUTE", p.cyan, false);
    pill(&mut frame, &p, 110, 168, 90, "CHARGER", p.green, true);

    let slots = [
        Some(("5:--", String::new(), 0.0, p.border)),
        Some(("6:--", String::new(), 0.0, p.border)),
        Some(("7:--", String::new(), 0.0, p.border)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    let active = [false; 8];
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

// ── EVENTS / MACRO / SETTINGS ──────────────────────────────────────────

/// Écran droit — vue EVENTS : événements du pattern (mesure sélectionnée).
#[must_use]
pub fn render_events_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let pat = active_pattern(snap);
    let (pat_name, total) = match pat {
        Some(pt) => (trunc(&pt.name, 12), pt.events.len()),
        None => ("--".to_string(), 0),
    };
    header(
        &mut frame,
        &p,
        "EVENTS",
        &format!(
            "GRP {} | {} | {} ev",
            group_letter(snap.group_focus),
            pat_name,
            total
        ),
    );

    let bar_offset = snap.selected_bar as u32 * TICKS_PER_BAR as u32;
    let mut events: Vec<&crate::project::PatternEvent> = pat
        .map(|pt| {
            pt.events
                .iter()
                .filter(|e| e.tick >= bar_offset && e.tick < bar_offset + TICKS_PER_BAR as u32)
                .collect()
        })
        .unwrap_or_default();
    events.sort_by_key(|e| e.tick);

    if events.is_empty() {
        center_text(&mut frame, 70, "AUCUN EVENEMENT", p.muted, p.bg);
        center_text(
            &mut frame,
            94,
            &format!("MES {} DU PATTERN", snap.selected_bar + 1),
            p.muted,
            p.bg,
        );
    }

    let mut y = 30usize;
    for (i, e) in events.iter().take(8).enumerate() {
        if i % 2 == 0 {
            fill_rect(&mut frame, 0, y - 2, SCR_W, 18, p.panel);
        }
        let step = ((e.tick - bar_offset) / TICKS_PER_STEP as u32) as usize;
        draw_text(
            &mut frame,
            8,
            y,
            &format!(
                "{:02} {}",
                e.pad + 1,
                trunc(&sound_name(snap, e.pad as usize), 10)
            ),
            p.white,
            p.panel,
        );
        draw_text(
            &mut frame,
            170,
            y,
            &format!("PAS {:02}", step + 1),
            p.dim,
            p.panel,
        );
        draw_text(
            &mut frame,
            250,
            y,
            &format!("VEL {:>3}", e.velocity),
            p.cyan,
            p.panel,
        );
        draw_text(
            &mut frame,
            340,
            y,
            &format!("LEN {}", e.length_ticks / TICKS_PER_STEP as u32),
            p.muted,
            p.panel,
        );
        draw_text(
            &mut frame,
            420,
            y,
            &format!("{:>3}", e.pitch),
            p.muted,
            p.panel,
        );
        y += 18;
    }
    if total > events.len() && !events.is_empty() {
        draw_text(
            &mut frame,
            8,
            y + 2,
            &format!("{} ev au total dans le pattern", total),
            p.muted,
            p.bg,
        );
    }

    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    let active = [false; 8];
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

/// Écran droit — vue MACRO : macros master / groupe.
#[must_use]
pub fn render_macro_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    header(&mut frame, &p, "MACRO", "MASTER / GROUPE");

    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let rows: [(&str, String, f32, DisplayPixel); 5] = [
        (
            "MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        ),
        ("VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan),
        ("VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan),
        (
            "SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.green,
        ),
        (
            "TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.green,
        ),
    ];
    let mut y = 34usize;
    for (label, value, norm, color) in &rows {
        draw_text(&mut frame, 8, y, label, p.dim, p.bg);
        bar_h(&mut frame, &p, 110, y, 300, 14, *norm, *color);
        let vx = SCR_W - 10 - value.len() * 8;
        draw_text(&mut frame, vx, y, value, p.white, p.bg);
        y += 30;
    }

    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    let active = [false; 8];
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

/// Écran droit — vue SETTINGS : infos projet / transport / modes.
#[must_use]
pub fn render_settings_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::new();
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    header(&mut frame, &p, "SETTINGS", "PROJET / TRANSPORT");

    let mut modes = Vec::new();
    if snap.scene_mode {
        modes.push("SCENE");
    }
    if snap.pattern_mode {
        modes.push("PATTERN");
    }
    if snap.select_mode {
        modes.push("SELECT");
    }
    if snap.erase_mode {
        modes.push("ERASE");
    }
    if modes.is_empty() {
        modes.push("JEU");
    }
    let ctrl = match snap.controller_mode {
        1 => "KEYBOARD",
        2 => "CHORDS",
        3 => "STEP",
        _ => "PAD",
    };
    let pad_map = std::env::var("MASCHINE_PAD_MAP").unwrap_or_else(|_| "hw".to_string());

    let lines = [
        format!("PROJET {}", trunc(&snap.project.name, 24)),
        format!(
            "{} GROUPES | {} SCENES | {} SECTIONS",
            snap.project.groups.len(),
            snap.project.scenes.len(),
            snap.project.song.len()
        ),
        format!("SCHEMA v{}", snap.project.schema_version),
        format!(
            "MASTER {:.0}% | CASQUE {:.0}%",
            snap.master_volume * 100.0,
            snap.headphone_volume * 100.0
        ),
        format!(
            "TRANSPORT {} | {:.1} BPM | TICK {}",
            if snap.transport.is_playing() {
                "PLAY"
            } else {
                "STOP"
            },
            snap.transport.bpm,
            snap.transport.tick
        ),
        format!(
            "LOOP {} | METRO {} | SWING {:.0}%",
            if snap.transport.loop_enabled {
                "ON"
            } else {
                "OFF"
            },
            if snap.transport.metronome {
                "ON"
            } else {
                "OFF"
            },
            snap.project.swing * 100.0
        ),
        format!("MODES PADS: {} | JEU: {ctrl}", modes.join("+")),
        format!("PAD MAP: {pad_map}"),
    ];
    let mut y = 30usize;
    for line in &lines {
        draw_text(&mut frame, 8, y, line, p.dim, p.bg);
        y += 18;
    }

    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    let active = [false; 8];
    footer_softkeys(&mut frame, &p, &snap.soft_labels, 4, &active);
    frame
}

// ── Dispatch ───────────────────────────────────────────────────────────

/// Rend la paire d'écrans (gauche, droite) selon la vue/onglets courants.
/// Partagée par le thread d'affichage USB et le miroir GUI (mêmes écrans).
#[must_use]
pub fn render_screens(snap: &EngineSnapshot) -> (DisplayFrame, DisplayFrame) {
    let left = if snap.song_mode {
        render_arranger_left(snap)
    } else if snap.scene_mode && snap.left_tab == 0 {
        render_scenes_left(snap)
    } else {
        match snap.left_tab {
            1 => render_mixer_left(snap),
            2 => render_sampler_left(snap),
            3 => render_browser_left(snap),
            _ => render_daw_left(snap),
        }
    };
    let right = if snap.song_mode {
        render_arranger_right(snap)
    } else if snap.left_tab == 2 && snap.right_tab == 0 {
        render_sampler_right(snap)
    } else if snap.left_tab == 3 && snap.right_tab == 0 {
        render_browser_right(snap)
    } else if snap.left_tab == 0 && snap.right_tab == 0 && snap.controller_mode == 3 {
        render_step_right(snap)
    } else {
        match snap.right_tab {
            1 => render_events_right(snap),
            2 => render_macro_right(snap),
            3 => render_settings_right(snap),
            _ => render_daw_right(snap),
        }
    };
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;

    fn check_pair(frame: &DisplayFrame) {
        assert_eq!(frame.w as usize, SCR_W);
        assert_eq!(frame.h as usize, SCR_H);
        assert_eq!(frame.pixels.len(), SCR_W * SCR_H);
        // Au moins le bandeau doit avoir été dessiné (pas un écran vide).
        assert!(frame
            .pixels
            .iter()
            .any(|px| *px != DisplayPixel::from_rgb(12, 13, 17)));
    }

    /// Toutes les vues des 2 écrans doivent se rendre sans panic.
    #[test]
    fn all_screens_render_without_panic() {
        let mut eng = Engine::new_demo(48000);
        eng.advance(4800);
        {
            let snap = eng.snapshot();
            let (l, r) = render_screens(&snap);
            check_pair(&l);
            check_pair(&r);
        }
        // Toutes les combinaisons d'onglets.
        for left_tab in 0..4u8 {
            for right_tab in 0..4u8 {
                let mut snap = eng.snapshot();
                snap.left_tab = left_tab;
                snap.right_tab = right_tab;
                snap.controller_mode = if left_tab == 0 { 3 } else { 0 };
                let (l, r) = render_screens(&snap);
                check_pair(&l);
                check_pair(&r);
            }
        }
        // Song, scènes, waveform, niveaux extrêmes (peak > 1.0).
        let mut snap = eng.snapshot();
        snap.song_mode = true;
        let (l, r) = render_screens(&snap);
        check_pair(&l);
        check_pair(&r);
        snap.song_mode = false;
        snap.scene_mode = true;
        snap.left_tab = 0;
        let (l, r) = render_screens(&snap);
        check_pair(&l);
        check_pair(&r);
        snap.scene_mode = false;
        snap.left_tab = 2;
        snap.right_tab = 0;
        snap.waveform = Some(
            (0..120)
                .map(|i| {
                    let v = (i as f32 * 0.21).sin() * 0.9;
                    (-v.abs(), v.abs())
                })
                .collect(),
        );
        snap.sample_duration = 0.53;
        snap.group_levels = [1.0; 8];
        snap.group_peaks = [1.2; 8];
        snap.master_level = 1.0;
        snap.master_peak = 1.5;
        let (l, r) = render_screens(&snap);
        check_pair(&l);
        check_pair(&r);
    }
}
