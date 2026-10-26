import java.util.ArrayList;
import java.util.List;
abstract class RetainedProcessor<T> {
    abstract int list(List<? extends T> values);
    abstract int array(T[] values);
}
class RetainedStringProcessor extends RetainedProcessor<String> {
    int list(List<? extends String> values) { return values.size(); }
    int array(String[] values) { return values.length; }
}
interface RetainedVisitor<T> { int visit(List<T> values); }
class RetainedStringVisitor implements RetainedVisitor<String> {
    public int visit(List<String> values) { return values.size(); }
}
class RetainedDispatchArgumentProbe {
    static List<String> savedList;
    static String[] savedArray;
    static int run() {
        List<String> values = new ArrayList<String>();
        values.add("a");
        savedList = values;
        String[] array = new String[] {"a", "b"};
        savedArray = array;
        RetainedProcessor<String> processor = new RetainedStringProcessor();
        RetainedVisitor<String> visitor = new RetainedStringVisitor();
        if (processor.list(values) != 1 || processor.array(array) != 2 || visitor.visit(values) != 1) return 1;
        savedList.add("b");
        savedArray[0] = "c";
        if (processor.list(values) != 2 || visitor.visit(values) != 2 || !array[0].equals("c")) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
