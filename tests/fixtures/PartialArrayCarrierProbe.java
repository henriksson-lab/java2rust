class PartialMatrixStore {
    double[][] rows = new double[3][];
    PartialMatrixStore(double[][] rows) { this.rows = rows; }
    PartialMatrixStore same() { return this; }
}
class PartialCubeStore {
    double[][][] matrices = new double[2][][];
    PartialCubeStore(double[][][] matrices) { this.matrices = matrices; }
    PartialCubeStore same() { return this; }
}
class PartialArrayCarrierProbe {
    static double[] row(double value) { return new double[] { value, value + 1 }; }
    static int run() {
        double[][] rows = new double[3][];
        double[] first = row(3);
        rows[0] = first;
        rows[1] = row(7);
        PartialMatrixStore store = new PartialMatrixStore(rows);
        PartialMatrixStore alias = store.same();
        if (store.rows != rows || store.rows[0] != first || rows[2] != null) return 1;
        first[0] = 11;
        if (alias.rows[0][0] != 11) return 2;
        alias.rows[1] = first;
        if (rows[1] != first) return 3;
        alias.rows[0] = null;
        if (rows[0] != null) return 4;
        try { double ignored = alias.rows[0][0]; return 5; }
        catch (NullPointerException expected) {}
        double[][][] cube = new double[2][][];
        cube[0] = rows;
        PartialCubeStore cubes = new PartialCubeStore(cube);
        if (cubes.same().matrices != cube || cubes.matrices[0] != rows || cube[1] != null) return 6;
        cubes.matrices[0][1][1] = 19;
        if (first[1] != 19) return 7;
        cube[0] = null;
        if (cubes.matrices[0] != null) return 8;
        try { int ignored = cubes.matrices[0].length; return 9; }
        catch (NullPointerException expected) {}
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
