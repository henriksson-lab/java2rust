package examples.expressions;

public class ExpressionPrecedence {
    public int compute(int a, int b, int c) {
        int x = a + b * c;
        int y = (a + b) * c;
        int z = x << 1;
        return z > y ? z - y : y - z;
    }
}

