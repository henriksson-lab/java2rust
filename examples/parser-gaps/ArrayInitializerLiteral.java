package parser.gaps;

public class ArrayInitializerLiteral {
    public double[] oneDimensional() {
        double[] values = {0, 1, +2.0, 3.1, -4, 5 * 10};
        return values;
    }

    public double[][] twoDimensional() {
        double[][] values = {{0, 1, +2.0}, {3.1, -4, 5}};
        return values;
    }
}
