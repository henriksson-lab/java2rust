class SourceArrayParameterContractProbe {
    double[] factors = new double[1];
    double[][] matrix = new double[][] { new double[] {2.0, 2.0} };
    void clear() { factors = null; matrix = null; }
    private void scale(double[] factors, double[][] array, int t) {
        double[] row = array[t];
        double sum = 0.0;
        for (int i = 0; i < row.length; i++) sum += row[i];
        factors[t] = sum;
        for (int i = 0; i < row.length; i++) row[i] /= sum;
    }
    int check() {
        double[] alias = matrix[0];
        scale(factors, matrix, 0);
        if (factors[0] != 4.0) return 1;
        if (alias[0] != 0.5 || alias[1] != 0.5) return 2;
        return 0;
    }
    static int run() { return new SourceArrayParameterContractProbe().check(); }
    public static void main(String[] args) { System.out.println(run()); }
}
