use gizza_ai_typescript_to_json_schema_core::{convert, Options};
fn main() {
    let src = r#"/** A support ticket. */
interface Ticket {
  id: number;
  title: string;
  status: "open" | "pending" | "closed";
  /** @format email */
  reporter: string;
  labels?: string[];
  assignee: Person | null;
}

interface Person {
  name: string;
  email: string;
}"#;
    println!("{}", convert(src, &Options::default()).unwrap());
}
