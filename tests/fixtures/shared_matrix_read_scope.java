interface MatrixView { double cell(); }
class MatrixUse {
    static double read(MatrixView value) { return value.cell(); }
}
class SharedMatrix {
    static int run() { return MatrixOwner.run(); }
    static int rowOrder() { return MatrixOwner.rowOrder(); }
    static int nullOrder() { return MatrixOwner.nullOrder(); }
}
class MatrixOwner implements MatrixView {
    double[][] data = new double[1][1];
    int calls;
    public double cell() { return data[0][0]; }
    int row() { calls++; return 0; }
    int column() { calls++; data = new double[][] {{9.0}}; return 0; }
    double probe() { return data[0][column()] + MatrixUse.read(this); }
    double bad() { return data[2][column()]; }
    double nullRead() { return data[row()][column()]; }
    static int run() {
        MatrixOwner owner = new MatrixOwner();
        owner.data[0][0] = 3.0;
        return (int) owner.probe();
    }
    static int rowOrder() {
        MatrixOwner owner = new MatrixOwner();
        try { owner.bad(); }
        catch (ArrayIndexOutOfBoundsException ex) { return owner.calls; }
        return -1;
    }
    static int nullOrder() {
        MatrixOwner owner = new MatrixOwner();
        owner.data = null;
        try { owner.nullRead(); }
        catch (NullPointerException ex) { return owner.calls; }
        return -1;
    }
}
