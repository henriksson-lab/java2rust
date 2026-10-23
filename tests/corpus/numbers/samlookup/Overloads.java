package samlookup;

public class Overloads {
    public static boolean same(float left, float right, float epsilon) { return false; }
    public static boolean same(double left, double right, double epsilon) {
        return left == right && epsilon >= 0;
    }
}
