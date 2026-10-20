public class RetainedMapViewsOracle {
    public static void main(String[] args) {
        int result = RetainedMapViews.run();
        if (result != 0) throw new AssertionError("retained map/view contract: " + result);
        System.out.println(result);
    }
}
