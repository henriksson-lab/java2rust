public class ArrayIndexException {
    public static String run() {
        int[] a = new int[2];
        try {
            return "got:" + a[5];
        } catch (ArrayIndexOutOfBoundsException e) {
            return "aioobe";
        }
    }
    public static void main(String[] args) { System.out.println(run()); }
}
