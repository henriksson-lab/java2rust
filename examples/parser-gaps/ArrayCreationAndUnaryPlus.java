package parser.gaps;

public class ArrayCreationAndUnaryPlus {
    public double unaryPlus() {
        return +2.0;
    }

    public double[] oneDimensional() {
        double[] values = new double[6];
        values[0] = 2.0;
        return values;
    }

    public double[][] twoDimensional() {
        double[][] values = new double[2][3];
        values[0][0] = 2.0;
        return values;
    }
}
