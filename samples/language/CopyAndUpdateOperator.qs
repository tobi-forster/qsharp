/// # Sample
/// Copy and Update Operator
///
/// # Description
/// The copy and update operator in Q# is used to make a copy of a data
/// structure, like an array or UDT, and update a single element in the
/// copied version of the structure.
namespace MyQuantumApp {

    newtype Pair = (first : Int, second : Int);

    @EntryPoint()
    operation Main() : Unit {
        let array = [10, 11, 12, 13];
        let pair = Pair(20, 21);

        // `w/` followed by the `<-` copies and updates a single element.

        // `new_array` is an array with values `[10, 11, 100, 13]`.
        // `array` is unchanged.
        let new_array = array w/ 2 <- 100;
        Message($"Updated array: {new_array}");

        // `new_array` is an array with values `[10, 100, 12, 200]`.
        // `array` is unchanged.
        let new_array = array
            w/ 1 <- 100
            w/ 3 <- 200;
        Message($"Updated array: {new_array}");

        // `new_pair` is a Pair with value `Pair(20, 100)`.
        // `pair` is unchanged.
        let new_pair = pair w/ second <- 100;
        Message($"Updated struct: (first:{new_pair::first}, second:{new_pair::second})");
    }
}
