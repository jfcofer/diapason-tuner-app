//! `dsp::note` against the table it shares with `core_domain` (`docs/adr/0007`). The table was
//! computed apart from both implementations, so this is an accuracy test, not a copy of the code.

use diapason_dsp::note::nearest;
use serde_json::Value;

const TABLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../fixtures/note_table.json"
));

/// The table holds six decimal places, so half of the sixth, and a little for the arithmetic.
const CENTS_TOLERANCE: f64 = 1e-6;

#[test]
fn every_row_of_the_shared_table_holds() {
    let table: Value = serde_json::from_str(TABLE).expect("the table is JSON");
    let rows = table["rows"].as_array().expect("the table has rows");
    assert!(rows.len() > 100, "the table lost its rows: {}", rows.len());

    for row in rows {
        let a4 = row["a4_hz"].as_f64().expect("a4_hz");
        let frequency = row["frequency_hz"].as_f64().expect("frequency_hz");
        let transposition = i32::try_from(row["transposition"].as_i64().expect("transposition"))
            .expect("a transposition is small");
        let note = i32::try_from(row["midi"].as_i64().expect("midi")).expect("a note is small");
        let cents = row["cents"].as_f64().expect("cents");

        let got = nearest(frequency, a4, transposition).expect("a positive frequency has a note");
        assert_eq!(got.note, note, "the note, at {row}");
        assert!(
            (got.cents - cents).abs() <= CENTS_TOLERANCE,
            "the cents, at {row}: got {}",
            got.cents
        );
    }
}
