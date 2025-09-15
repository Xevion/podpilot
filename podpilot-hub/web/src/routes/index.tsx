import { createFileRoute } from "@tanstack/react-router";
import { Card, Flex, Text } from "@radix-ui/themes";
import { ThemeToggle } from "../components/ThemeToggle";
import "../App.css";

function App() {
  return (
    <div className="App">
      <div
        style={{
          position: "fixed",
          top: "20px",
          right: "20px",
          zIndex: 1000,
        }}
      >
        <ThemeToggle />
      </div>

      <Flex
        direction="column"
        align="center"
        justify="center"
        style={{ minHeight: "100vh", padding: "20px" }}
      >
        <Card>
          <Flex direction="column" gap="4">
            <Text>Hello, world!</Text>
          </Flex>
        </Card>
      </Flex>
    </div>
  );
}

export const Route = createFileRoute("/")({
  component: App,
});
