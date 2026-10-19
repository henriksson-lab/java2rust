// Compile alongside tests/fixtures/retained_remove_all.java.
public class RetainedRemoveAllOracle {
    public static void main(String[] args) {
        System.out.println(RetainedRemoveAll.plainReceiver() + " "
            + RetainedRemoveAll.retainedReceiver() + " " + RetainedRemoveAll.plainList()
            + " " + RetainedRemoveAll.selfRemoval());
    }
}
