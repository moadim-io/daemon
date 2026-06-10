use std::fs;
use std::path::Path;

pub fn generate(manifest_dir: &str) {
    let out_path = Path::new(manifest_dir).join("apis/graphql.graphql");

    // SDL must stay in sync with src/routes/graphql.rs.
    // Field names follow async-graphql's default snake_case → camelCase conversion.
    // Timestamps (createdAt, updatedAt, uptimeSecs) are Unix seconds as Int.
    let sdl = r#""""Arbitrary JSON value (object, array, or primitive)"""
scalar JSON

"""A managed or read-only system cron job"""
type CronJob {
  id: String!
  schedule: String!
  handler: String!
  metadata: JSON!
  enabled: Boolean!
  """\"managed\" for server-owned jobs; \"system:*\" for read-only system entries"""
  source: String!
  """Unix timestamp (seconds)"""
  createdAt: Int!
  """Unix timestamp (seconds)"""
  updatedAt: Int!
}

"""A cron job with its handler registration status"""
type CronJobResponse {
  id: String!
  schedule: String!
  handler: String!
  metadata: JSON!
  enabled: Boolean!
  source: String!
  """Unix timestamp (seconds)"""
  createdAt: Int!
  """Unix timestamp (seconds)"""
  updatedAt: Int!
  """True if the handler name matches a registered handler on this server"""
  handlerRegistered: Boolean!
}

type Health {
  status: String!
  """Seconds since server start"""
  uptimeSecs: Int!
  running: Boolean!
}

type Query {
  """List all managed cron jobs with handler registration status"""
  cronJobs: [CronJobResponse!]!
  """Get a managed cron job by ID; returns null if not found"""
  cronJob(id: String!): CronJobResponse
  """List read-only system cron jobs from crontab and /etc/cron.d"""
  systemCronJobs: [CronJob!]!
  """Server health and uptime"""
  health: Health!
}

type Mutation {
  """Create a new managed cron job"""
  createCronJob(input: CreateCronJobInput!): CronJob!
  """Update one or more fields of an existing managed cron job"""
  updateCronJob(id: String!, input: UpdateCronJobInput!): CronJob!
  """Delete a managed cron job by ID; returns the deleted job ID"""
  deleteCronJob(id: String!): String!
}

input CreateCronJobInput {
  schedule: String!
  handler: String!
  metadata: JSON
  """Defaults to true"""
  enabled: Boolean! = true
}

input UpdateCronJobInput {
  schedule: String
  handler: String
  metadata: JSON
  enabled: Boolean
}
"#;

    fs::write(&out_path, sdl).expect("failed to write apis/graphql.graphql");
}
