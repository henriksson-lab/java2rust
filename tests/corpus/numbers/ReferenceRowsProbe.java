public final class ReferenceRowsProbe {
    static void rotate(double[] key, double[]... rows) {
        for (double[] row : rows) {
            if (row.length != key.length) throw new IllegalArgumentException("length");
        }
        key[0]++;
        for (double[] row : rows) {
            double first = row[0]; row[0] = row[1]; row[1] = first;
        }
    }
    public static int run() {
        double[] key = {1,2}; double[] y = {10,20};
        rotate(key,y,y);
        if (key[0] != 2 || key[1] != 2 || y[0] != 10 || y[1] != 20) return 1;
        key = new double[]{3,8}; y = new double[]{30,40};
        rotate(key,key,y,key);
        if (key[0] != 4 || key[1] != 8 || y[0] != 40 || y[1] != 30) return 2;
        key = new double[]{5,9}; y = new double[]{10,20};
        double[][] rows = {y,key,y};
        rotate(key,rows);
        if (key[0] != 9 || key[1] != 6 || y[0] != 10 || y[1] != 20) return 3;
        rows[0] = key;
        rotate(key,rows);
        if (key[0] != 10 || key[1] != 6 || y[0] != 20 || y[1] != 10) return 4;
        try { rotate(key,y,new double[]{1}); return 5; }
        catch (IllegalArgumentException expected) { }
        if (key[0] != 10 || key[1] != 6 || y[0] != 20 || y[1] != 10) return 6;
        try { rotate(key,y,(double[])null); return 7; }
        catch (NullPointerException expected) { }
        if (key[0] != 10 || key[1] != 6 || y[0] != 20 || y[1] != 10) return 8;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
