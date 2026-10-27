class Unrelated<Cell> {
    Cell value;
    Unrelated(Cell value) { this.value = value; }
    Cell get() { return value; }
}

class ScopedValueReferences {
    static <Cell> java.util.function.IntSupplier forward(java.util.function.IntSupplier Cell) {
        return Cell::getAsInt;
    }
    static int run() {
        return ScopedValueReferences.<Cell>forward(() -> 13).getAsInt();
    }
}
