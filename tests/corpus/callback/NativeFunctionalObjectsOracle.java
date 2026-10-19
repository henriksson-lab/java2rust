public class NativeFunctionalObjectsOracle {
    public static void main(String[] arguments) {
        assert NativeFunctionalObjects.sampled() == 345;
        assert NativeFunctionalObjects.sampledInherited() == 8;
        assert NativeFunctionalObjects.compared() == 1;
        assert NativeFunctionalObjects.fieldConversion() == 1;
        assert NativeFunctionalObjects.replacedField() == 9;
        assert NativeFunctionalObjects.throughField() == 9;
        assert NativeFunctionalObjects.joined().equals("firstsecond");
        assert NativeFunctionalObjects.nullable(false) == -1;
        assert NativeFunctionalObjects.nullable(true) == 5;
        assert NativeFunctionalObjects.nullSource() == 1;
        assert NativeFunctionalObjects.sharedObject() == 78;
        assert NativeFunctionalObjects.caught() == 9;
        assert NativeFunctionalAlias.sharedCounter() == 345;
        System.out.println("native-functional-objects: ok");
    }
}
