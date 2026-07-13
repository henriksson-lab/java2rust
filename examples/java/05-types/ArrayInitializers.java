package examples.types;

public class ArrayInitializers {
    public double[] oneDimensional(double[] values) {
        values[0] = 0;
        values[1] = 1;
        values[2] = 2.0;
        values[3] = 3.1;
        values[4] = -4;
        values[5] = 5 * 10;
        return values;
    }

    public double[][] twoDimensional(double[][] values) {
        values[0][0] = 0;
        values[0][1] = 1;
        values[0][2] = 2.0;
        values[1][0] = 3.1;
        values[1][1] = -4;
        values[1][2] = 5;
        return values;
    }
}
