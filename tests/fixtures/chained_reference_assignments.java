class AssignmentNode { int value; }
class ChainedReferenceAssignments {
    static boolean clear() {
        AssignmentNode[] values = new AssignmentNode[] { new AssignmentNode(), new AssignmentNode() };
        AssignmentNode assigned = (values[0] = values[1] = null);
        return assigned == null && values[0] == null && values[1] == null;
    }
    static int fill() {
        AssignmentNode[] values = new AssignmentNode[2];
        AssignmentNode assigned = (values[0] = values[1] = new AssignmentNode());
        assigned.value = 7;
        return values[0].value + values[1].value;
    }
    static boolean strings() {
        String[] values = new String[2];
        String assigned = (values[0] = values[1] = new String("shared"));
        return assigned == values[0] && values[0] == values[1];
    }
}
