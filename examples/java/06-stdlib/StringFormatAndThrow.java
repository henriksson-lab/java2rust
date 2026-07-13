package examples.stdlib;

public class StringFormatAndThrow {
    private static final int MIN_ADDRESS = 16;
    private static final int MAX_ADDRESS = 126;

    public static String describe(double strength, double angle) {
        return String.format("signal %3.0f%% at %6.1f degrees", strength * 100.0d, angle);
    }

    public static void requireEvenAddress(int address) {
        if (address < MIN_ADDRESS || address > MAX_ADDRESS) {
            throw new IllegalArgumentException(String.format("address %d outside %d..%d", address, MIN_ADDRESS, MAX_ADDRESS));
        }
        if ((address % 2) != 0) {
            throw new IllegalArgumentException(String.format("address %d must be even", address));
        }
    }
}
