import { createMemoryHistory, createRouter, createWebHashHistory } from 'vue-router'
import AboutPage from '../pages/AboutPage.vue'
import DiscoverPage from '../pages/DiscoverPage.vue'
import DownloadsPage from '../pages/DownloadsPage.vue'
import HomePage from '../pages/HomePage.vue'
import HistoryPage from '../pages/HistoryPage.vue'
import LibraryPage from '../pages/LibraryPage.vue'
import PlaylistsPage from '../pages/PlaylistsPage.vue'
import ProfilePage from '../pages/ProfilePage.vue'
import SearchPage from '../pages/SearchPage.vue'
import SettingsPage from '../pages/SettingsPage.vue'

export const routes = [
  { path: '/', name: 'home', component: HomePage, meta: { title: 'Listen' } },
  { path: '/discover', name: 'discover', component: DiscoverPage, meta: { title: 'Discover' } },
  { path: '/search', name: 'search', component: SearchPage, meta: { title: 'Search' } },
  { path: '/library', name: 'library', component: LibraryPage, meta: { title: 'Library' } },
  { path: '/playlists', name: 'playlists', component: PlaylistsPage, meta: { title: 'Playlists' } },
  { path: '/history', name: 'history', component: HistoryPage, meta: { title: 'History' } },
  { path: '/downloads', name: 'downloads', component: DownloadsPage, meta: { title: 'Downloads' } },
  { path: '/settings', name: 'settings', component: SettingsPage, meta: { title: 'Settings' } },
  { path: '/profile', name: 'profile', component: ProfilePage, meta: { title: 'Profile' } },
  { path: '/about', name: 'about', component: AboutPage, meta: { title: 'About' } },
  { path: '/:pathMatch(.*)*', redirect: { name: 'home' } },
]

export function createAppRouter(history = typeof window === 'undefined' ? createMemoryHistory() : createWebHashHistory()) {
  return createRouter({ history, routes })
}

export const router = createAppRouter()