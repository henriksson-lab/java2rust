package org.demo;
import java.util.List;
import java.util.Map;
import org.external.Widget;
public class Store {
    public int count;
    public Map<String, Integer> counts;            // generic field
    public Store(String name) {}
    @Nullable public String lookup(@Nullable String key) { return null; }
    public void register(String name, int times) {}
    public static Store create() { return null; }
    public List<String> names() { return null; }    // List<String> -> Vec<String>
    public Map<String, Widget> index() { return null; }   // Map -> HashMap<String, Widget>
    public <T> T pick(List<T> xs) { return null; }  // type variable
    public List<? extends Number> nums() { return null; } // wildcard bound
    public Widget makeWidget() { return null; }     // type NOT in the jar
    private void secret() {}                          // private: skipped
}
