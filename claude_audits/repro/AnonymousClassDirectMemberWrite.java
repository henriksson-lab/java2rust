// Anonymous class-carrier boundary: the override must mutate the same Node
// observed by the factory caller, without falling back to a nominal Base.
class Node {
    int value;
    Node(int value) { this.value = value; }
}

class Base {
    Base() {}
    int write(boolean enabled, int value) { return value; }
}

class AnonymousClassDirectMemberWrite {
    static Base make(Node incoming) {
        return new Base() {
            Node node = incoming;
            @Override int write(boolean enabled, int value) {
                int next = value + 1;
                if (enabled) {
                    this.node.value = next;
                } else {
                    this.node.value = next + 2;
                }
                return this.node.value;
            }
        };
    }

    static int run() {
        Node node = new Node(3);
        Base value = make(node);
        value.write(true, 23);
        return node.value;
    }
}
