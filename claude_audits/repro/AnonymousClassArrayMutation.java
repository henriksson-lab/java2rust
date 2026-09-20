class ArrayWriteBase {
    ArrayWriteBase() {}

    int bump(int[] values) {
        values[0] = values[0] + 1;
        return values[0];
    }
}

public class AnonymousClassArrayMutation {
    static ArrayWriteBase make() {
        return new ArrayWriteBase() {};
    }

    public static void main(String[] args) {
        int[] values = {4};
        ArrayWriteBase value = make();
        System.out.println(value.bump(values) + values[0]);
    }
}
