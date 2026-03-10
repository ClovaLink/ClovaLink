# Document Approval Workflow

ClovaLink's Document Approval Workflow lets organizations require uploaded files to be reviewed and approved before they become accessible to other users. The feature is opt-in per tenant, fully configurable through approval policies, and integrates with existing role-based permissions.

## Enabling the Feature

1. Navigate to **Company Settings** (gear icon in sidebar).
2. Open the **Document Workflow** tab.
3. Toggle **Require approval for file uploads** to **On**.
4. Click **Save**.

When the feature is enabled, the sidebar shows an **Approvals** link for users with the `approvals.view` permission (Manager and above by default). When disabled, the link is hidden and all uploads are treated as approved.

> **Note:** Enabling approval workflow does not retroactively affect existing files. All files uploaded before activation default to `approved`.

## Approval Policies

Approval policies define *which* uploads require approval. Policies are managed by Admins under **Company Settings > Document Workflow**. Each policy has a **scope** that determines what it matches against.

| Scope | `scope_value` | What It Matches |
|-------|---------------|-----------------|
| **all** | *(none)* | Every file uploaded to the tenant. Catch-all; evaluated last. |
| **department** | Department UUID | Files uploaded to a specific department. |
| **company_folder** | *(none)* | Files uploaded to any company folder (`is_company_folder = true`). |
| **file_type** | Comma-separated extensions (e.g. `pdf,docx,xlsx`) | Files whose extension matches any in the list. Case-insensitive. |
| **file_size** | Byte threshold (e.g. `10485760` for 10 MB) | Files whose size is greater than or equal to the threshold. |
| **role** | Role name (e.g. `Employee`) | Files uploaded by users with the specified base role. |
| **private_files** | *(none)* | Files with `visibility = 'private'`. |

### Evaluation Order

When a file is uploaded, the system evaluates policies in the following priority:

1. **Specific scopes** -- `department`, `company_folder`, `file_type`, `file_size`, `role`, `private_files` are checked first. The first match wins.
2. **Catch-all** -- If no specific scope matched, the `all` scope is checked last.
3. **No match** -- If no active policy matches the file, it is uploaded with status `approved` (no review needed).

Only **active** policies are evaluated. You can temporarily disable a policy without deleting it by toggling its `is_active` flag.

## Automatic Approval Flow

When the approval workflow is enabled and a user uploads a file:

1. The system calls `find_matching_policy` with the file's metadata (department, folder type, file name, size, visibility, uploader role).
2. If a matching active policy is found, the file's `approval_status` is set to **pending** and an `approval_request` record is created.
3. An **ApprovalRequired** notification is sent to all Managers and Admins in the tenant.
4. The file remains inaccessible for download, sharing, and locking until approved.

If no policy matches, the file is uploaded normally with status `approved`.

## Manual Approval -- Send for Approval

Users can manually send an already-approved file for re-review:

1. Open the file's action menu (three-dot icon).
2. Select **Send for Approval**.
3. The file status changes to **pending** and a new approval request is created.
4. Approvers are notified.

**Who can send:** The file owner, or any Admin/SuperAdmin.

**Preconditions:** The file must currently have status `approved`. Files that are already `pending` or `rejected` cannot be sent again through this action.

## Approving and Rejecting Files

Managers, Admins, and SuperAdmins can review pending files from the **Approvals** page, which has two tabs:

### Pending Tab

- Lists all files awaiting approval, ordered by submission date (newest first).
- Filterable by department.
- Each entry shows the file name, size, content type, uploader email/name, and submission date.
- **Approve** -- Sets the file status to `approved` and notifies the uploader.
- **Reject** -- Requires a reason (1--2000 characters). Sets the file status to `rejected` and notifies the uploader with the reason.

### History Tab

- Lists all completed approval requests (approved and rejected).
- Shows the decider's email and the decision timestamp.
- Rejected entries include the rejection reason.

### My Pending (Uploader View)

All users can view their own pending and rejected files via the **My Pending** section, which shows:

- File name, size, and content type
- Current status (`pending` or `rejected`)
- Rejection reason (if applicable)

## Resubmitting Rejected Files

When a file is rejected, the owner can resubmit it for another round of review:

1. Navigate to **My Pending** or locate the rejected file in your files list.
2. Click **Resubmit for Approval**.
3. The file status resets to **pending** and a new approval request is created.

**Only the file owner** can resubmit. The previous rejection reason remains in the approval history for audit purposes.

## Approval Statistics

The **Approvals** page displays summary statistics:

| Metric | Description |
|--------|-------------|
| Pending | Number of files currently awaiting approval |
| Approved | Total files approved to date |
| Rejected | Total files rejected to date |

## Role Permissions

| Permission | Default Roles | Description |
|------------|---------------|-------------|
| `approvals.view` | Manager, Admin, SuperAdmin | View the Approvals page (pending and history tabs) |
| `approvals.manage` | Manager, Admin, SuperAdmin | Approve or reject files |

Employees can view their own pending/rejected files but cannot access the main Approvals page or make decisions on other users' files.

Policy management (create, update, delete) requires **Admin** or **SuperAdmin** role.

## Notifications

The approval workflow triggers two notification types:

| Notification | Recipient | Trigger |
|-------------|-----------|---------|
| **ApprovalRequired** | All Admins (via `notify_all_admins`) | A file is uploaded and matches a policy, or is manually sent for approval |
| **ApprovalDecision** | The file uploader | A file is approved or rejected |

Both notifications support email delivery using configurable email templates:

- **`approval_required`** -- Variables: `user_name`, `file_name`, `uploader_name`, `company_name`, `app_url`
- **`approval_decision`** -- Variables: `file_name`, `decision`, `reason`, `company_name`

Users can control their notification preferences for these types in **Profile > Notifications**.

## API Endpoints

All endpoints are scoped under `/api/approvals/:company_id` and require authentication.

| Method | Path | Description | Required Role |
|--------|------|-------------|---------------|
| `GET` | `/pending` | List files pending approval (paginated, filterable by department) | Manager+ |
| `GET` | `/history` | List completed approval requests (paginated) | Manager+ |
| `GET` | `/my-pending` | List current user's pending/rejected files | Any authenticated user |
| `GET` | `/stats` | Get approval counts (pending, approved, rejected) | Manager+ |
| `POST` | `/:request_id/approve` | Approve a pending file | Manager+ |
| `POST` | `/:request_id/reject` | Reject a pending file (body: `{ "reason": "..." }`) | Manager+ |
| `POST` | `/:file_id/send` | Manually send an approved file for review | File owner or Admin+ |
| `POST` | `/:file_id/resubmit` | Resubmit a rejected file for approval | File owner only |
| `GET` | `/policies` | List all approval policies | Admin+ |
| `POST` | `/policies` | Create a new approval policy | Admin+ |
| `PUT` | `/policies/:id` | Update an existing policy | Admin+ |
| `DELETE` | `/policies/:id` | Delete an approval policy | Admin+ |

### Pagination

The `pending`, `history`, and `my-pending` endpoints accept query parameters:

- `page` -- Page number (0-indexed, default `0`)
- `limit` -- Items per page (default `50`, max `100`)
- `department_id` -- (pending only) Filter by department UUID

## Security Considerations

- **Tenant isolation**: All queries filter by `tenant_id`. Users cannot view or act on approvals from other tenants.
- **Atomic decisions**: Approve and reject operations use `WHERE status = 'pending'` in the UPDATE statement, preventing double-processing if two approvers act simultaneously.
- **Access gating**: Files with `approval_status = 'pending'` or `'rejected'` are blocked from download, sharing, and locking until approved.
- **Audit trail**: Every approval action (approve, reject, send, resubmit, policy create/delete) is recorded in the `audit_logs` table with the acting user, IP address, and relevant metadata.
- **Input validation**: Rejection reasons must be 1--2000 characters. Policy names must be 1--255 characters. Invalid scopes are rejected with 400 Bad Request.
- **Owner verification**: Only the file owner can resubmit rejected files. Only the owner or Admin+ can send files for manual approval.
