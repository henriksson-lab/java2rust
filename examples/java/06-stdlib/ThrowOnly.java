package examples.stdlib;

public class ThrowOnly {
    public int requirePositive(int value) throws IllegalArgumentException {
        if (value < 0) {
            throw new IllegalArgumentException("negative");
        }
        return value;
    }
}
