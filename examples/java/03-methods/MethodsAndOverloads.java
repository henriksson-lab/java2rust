package examples.methods;

public class MethodsAndOverloads {
    public static int clamp(int value, int min, int max) {
        if (value < min) {
            return min;
        }
        if (value > max) {
            return max;
        }
        return value;
    }

    public String format(int value) {
        return "int:" + value;
    }

    public String format(String value) {
        return "str:" + value;
    }
}

