import java.util.ArrayList;
import java.util.List;

class FlowObservation {}

class WildcardCollectionFlowProbe {
    <O extends FlowObservation> int compute(List<? extends O> values) {
        clearAlias(values);
        return count(values);
    }

    <O extends FlowObservation> void clearAlias(List<O> values) {
        List<O> alias = values;
        alias.clear();
    }

    <O extends FlowObservation> int count(List<O> values) {
        return values.size();
    }

    static int run() {
        WildcardCollectionFlowProbe probe = new WildcardCollectionFlowProbe();
        ArrayList<FlowObservation> values = new ArrayList<FlowObservation>();
        values.add(new FlowObservation());
        int count = probe.compute(values);
        return count == 0 && values.size() == 0 ? 0 : 1;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
