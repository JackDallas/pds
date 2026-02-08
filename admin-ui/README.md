# PDS Admin UI

Svelte 5 admin interface for dallas-pds multi-user Personal Data Server.

## Features

- **Dashboard**: Overview with account, repo, and invite statistics
- **Accounts**: List, search, and moderate user accounts (takedown/restore)
- **Invites**: Create and manage invite codes
- **Settings**: View PDS configuration (read-only)

## Technology Stack

- **SvelteKit 2.x** with adapter-static (SPA mode)
- **Svelte 5** with runes ($state, $derived, $effect)
- **Tailwind CSS 4.x** with Tailwind Vite plugin
- **TypeScript**

## Project Structure

```
admin-ui/
├── src/
│   ├── lib/
│   │   ├── auth.svelte.ts      # Auth state management with Svelte 5 runes
│   │   └── api.ts              # XRPC API client
│   ├── routes/
│   │   ├── +layout.svelte      # Main layout with sidebar navigation
│   │   ├── +layout.ts          # SPA mode configuration (prerender, no SSR)
│   │   ├── +page.svelte        # Login page
│   │   ├── dashboard/          # Dashboard page
│   │   ├── accounts/           # Account management page
│   │   ├── invites/            # Invite code management page
│   │   └── settings/           # Configuration view page
│   ├── app.html                # HTML template
│   └── app.css                 # Tailwind CSS imports
├── build/                      # Production build output (served at /admin)
├── svelte.config.js            # SvelteKit configuration
├── vite.config.ts              # Vite configuration
└── package.json
```

## Development

```bash
# Install dependencies
pnpm install

# Run development server
pnpm dev

# Build for production
pnpm build

# Preview production build
pnpm preview
```

## Configuration

- **Base Path**: `/admin` (configured in svelte.config.js)
- **Output**: Static files in `build/` directory
- **Fallback**: `index.html` for client-side routing

## API Integration

The UI communicates with the PDS via XRPC endpoints:

- `com.atproto.server.createSession` - Authentication
- `com.atproto.admin.getAccountInfo` - Get account details
- `tools.ozone.moderation.queryStatuses` - List accounts
- `com.atproto.admin.updateSubjectStatus` - Takedown/restore accounts
- `com.atproto.server.getAccountInviteCodes` - List invite codes
- `com.atproto.server.createInviteCode` - Create invite codes
- `com.atproto.admin.disableInviteCodes` - Disable invite codes
- `com.dallaspds.admin.getStats` - Get PDS statistics
- `com.dallaspds.admin.getConfig` - Get PDS configuration

## Authentication

- Admin users authenticate with their handle/email and password
- Access tokens are stored in localStorage
- Only accounts with `is_admin = true` can access the admin UI
- Logout clears localStorage and redirects to login

## Deployment

The `build/` directory contains static files that should be served at the `/admin` path by the PDS server. Configure your HTTP server to:

1. Serve files from `build/` at `/admin`
2. Route all `/admin/*` requests to `/admin/index.html` for client-side routing
3. Ensure proper CORS headers for API requests

## Notes

- Dark mode is enabled by default via Tailwind's `dark:` classes
- Client-side only (no SSR) - authentication state loads from localStorage on mount
- Uses Svelte 5's modern runes API ($state, $derived, $effect)
- No component library - pure Tailwind utility classes
