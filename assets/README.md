# Assets

## handwriting.bin

Examples of handwriting for the recognizer (`src/recognizer.rs`). The data comes from the **UJI Pen
Characters (Version 2)** database: 60 writers, 2 examples of each character for each writer.

- Source: https://archive.ics.uci.edu/dataset/404/uji+pen+characters+version+2
- Authors: David Llorens, Federico Prat, Andrés Marzal, Juan Miguel Vilar, María José Castro, Juan
  Carlos Amengual, Sergio Barrachina, Antonio Castellanos, Salvador España, Jon Ander Gómez, Jorge
  González, Alfons Juan, Vicente Palazón, María José Ramos, Rosa Sanchis, Alberto Sanchis, Enrique
  Vidal. "The UJIpenchars Database: a Pen-Based Database of Isolated Handwritten Characters", LREC
  2008.
- License: Creative Commons Attribution 4.0 International (CC BY 4.0).

Changes to the data: the characters with the shape of a 1 (`l`, `I`, `|`, `'`, `,`, `!`, `¡`, `i`,
`í`, `Í`, `j`) are removed. Each example is resampled to a maximum of approximately 40 points and
scaled to a box of 0 to 255. The digits 1 to 9 keep their class. All other characters have the class 0
("not 1–9").

To make the file again:

```sh
curl -O https://archive.ics.uci.edu/ml/machine-learning-databases/uji-penchars/version2/ujipenchars2.txt
cargo run --release --example build_handwriting -- ujipenchars2.txt
```

Format (little-endian): `HW01`, u16 quantity of examples, then for each example: u8 class (0 to 9),
u8 writer (1 to 60), u8 quantity of strokes, and for each stroke: u8 quantity of points, then u8 x and
u8 y for each point.
